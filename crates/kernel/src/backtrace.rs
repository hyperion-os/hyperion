use core::{arch::asm, ffi::c_void, ptr};

use elf::{
    endian::AnyEndian, string_table::StringTable, symbol::SymbolTable, ElfBytes, ParseError,
};
use rustc_demangle::demangle;
use spin::Lazy;
use x86_64::VirtAddr;

use crate::{
    arch,
    boot::{self, virt_addr},
    log::LogLevel,
    println,
    term::escape::encode::EscapeEncoder,
};

//

pub type BacktraceResult<T> = Result<T, BacktraceError>;

#[derive(Debug)]
pub enum BacktraceError {
    NoSymtabOrStrtab,
    ElfNotLoaded,
    ElfParse(ParseError),

    // TODO: this is temporary
    Inner(&'static Self),
}

#[derive(Debug, Clone, Copy)]
pub struct FrameInfo {
    pub instr_ptr: usize,
    pub symbol_name: &'static str,
    pub symbol_offs: Option<usize>,
}

//

pub fn print_symtab() -> Result<(), BacktraceError> {
    let (symtab, strtab) = SYMTAB.as_ref().map_err(BacktraceError::Inner)?;

    for sym in symtab.iter() {
        if let Ok(s_sym) = strtab.get(sym.st_name as _) {
            if s_sym.matches("hyperion").count() == 0 {
                continue;
            }

            let symname = demangle(s_sym);

            println!(
                "symtab sym `{symname}` at `{:#018x}` size: {}",
                sym.st_value, sym.st_size
            );
        }
    }

    Ok(())
}

/// returns the `symbol_name + offset` for the provided instruction pointer
///
/// or an error if the (sym/str)tab could not be read
pub fn symbol(instr_ptr: u64) -> Result<(&'static str, usize), BacktraceError> {
    let (symtab, strtab) = SYMTAB.as_ref().map_err(BacktraceError::Inner)?;

    let symbol = symtab
        .iter()
        .find(|sym| (sym.st_value..sym.st_value + sym.st_size).contains(&instr_ptr));

    let Some(symbol) = symbol else {
        return Ok((UNKNOWN, 0));
    };
    let offs = (instr_ptr - symbol.st_value) as usize;

    strtab
        .get(symbol.st_name as _)
        .map(|sym| (sym, offs))
        .map_err(BacktraceError::ElfParse)
}

pub fn symbol_noerr(instr_ptr: u64) -> FrameInfo {
    let (symbol_name, symbol_offs) = symbol(instr_ptr as _)
        .map(|(s, o)| (s, Some(o)))
        .unwrap_or((UNKNOWN, None));
    FrameInfo {
        instr_ptr: instr_ptr as _,
        symbol_name,
        symbol_offs,
    }
}

pub fn base_ptr() -> VirtAddr {
    let frame: u64;
    unsafe {
        // TODO: move to arch
        asm!("mov {}, rbp", out(reg) frame);
    }
    VirtAddr::new(frame)
}

pub fn kernel_base() -> u64 {
    unsafe { &KERNEL_BASE as *const c_void as _ }
}

/// # Safety
///
/// caller must ensure that `ip` points to a valid stack frame
/// and that stackframes end with a NULL
pub unsafe fn unwind_stack_from(ip: VirtAddr, mut f: impl FnMut(FrameInfo)) {
    struct RawStackFrame {
        next: *const RawStackFrame,
        instr_ptr: u64,
    }

    let mut frame: *const RawStackFrame = ip.as_u64() as _;

    arch::int::without(|| {
        loop {
            if frame.is_null() {
                break;
            }

            let _frame = unsafe { ptr::read_volatile(frame) };
            frame = _frame.next;

            if _frame.instr_ptr == 0 {
                break;
            }

            let instr_ptr = _frame
                .instr_ptr
                .checked_sub(virt_addr().as_u64())
                .and_then(|i| i.checked_add(kernel_base()))
                .unwrap_or(_frame.instr_ptr);
            f(symbol_noerr(instr_ptr));
        }

        println!("{:#0x}", frame as usize);
    });
}

pub fn unwind_stack(f: impl FnMut(FrameInfo)) {
    unsafe { unwind_stack_from(base_ptr(), f) }
}

/// # Safety
///
/// caller must ensure that `ip` points to a valid stack frame
/// and that stackframes end with a NULL
pub unsafe fn print_backtrace_from(ip: VirtAddr) {
    crate::log::print_log_splash(
        LogLevel::Info,
        " BACKTRACE".true_yellow(),
        "begin",
        format_args_nl!(""),
    );
    let mut i = 0usize;
    unwind_stack_from(
        ip,
        |FrameInfo {
             instr_ptr: ip,
             symbol_name: sym,
             symbol_offs: offs,
         }| {
            let sym = demangle(sym);
            if let Some(offs) = offs {
                println!(
                    "{i:>4}: {:#018x}+{:<4x} => {sym}",
                    ip.true_cyan(),
                    offs.true_cyan()
                );
            } else {
                println!("{i:>4}: {:#018x} - {sym}", ip.true_cyan());
            }
            i += 1;
        },
    );
    crate::log::print_log_splash(
        LogLevel::Info,
        " BACKTRACE".true_yellow(),
        "end",
        format_args_nl!(""),
    );
}

pub fn print_backtrace() {
    unsafe { print_backtrace_from(base_ptr()) }
}

//

static KERNEL_ELF: Lazy<BacktraceResult<ElfBytes<'static, AnyEndian>>> = Lazy::new(|| {
    let bytes = boot::kernel_file().ok_or(BacktraceError::ElfNotLoaded)?;
    ElfBytes::minimal_parse(bytes).map_err(BacktraceError::ElfParse)
    // ElfBytes::minimal_parse(bytes)
    //     .ok()
    //     .ok_or(BacktraceError::ElfParse)
});

static SYMTAB: Lazy<
    Result<(SymbolTable<'static, AnyEndian>, StringTable<'static>), BacktraceError>,
> = Lazy::new(|| {
    let elf = KERNEL_ELF.as_ref().map_err(BacktraceError::Inner)?;

    elf.symbol_table()
        .map_err(BacktraceError::ElfParse)?
        .ok_or(BacktraceError::NoSymtabOrStrtab)
});

static UNKNOWN: &str = "<unknown>";

extern "C" {
    static KERNEL_BASE: c_void;
}
