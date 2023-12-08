#![no_std]
#![feature(naked_functions)]

//

use core::{arch::asm, ffi::c_void, ptr};

use elf::{
    endian::AnyEndian, string_table::StringTable, symbol::SymbolTable, ElfBytes, ParseError,
};
use hyperion_boot::{kernel_file, virt_addr};
use hyperion_escape::encode::EscapeEncoder;
use hyperion_log::{println, LogLevel};
use rustc_demangle::demangle;
use spin::Lazy;
use x86_64::VirtAddr;

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

/* #[derive(Debug)]
#[repr(C)]
pub struct SavedRegs {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
}

#[derive(Debug)]
#[repr(C)]
pub struct UnwindRegs {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
    rsp: u64,
    ra: u64,
}

pub fn current_registers(mut f: impl FnMut(UnwindRegs)) {
    let mut f: &mut dyn FnMut(UnwindRegs) = &mut f;
    save_current_registers(&mut f);

    #[naked]
    extern "sysv64" fn save_current_registers(_f: *mut &mut dyn FnMut(UnwindRegs)) {
        unsafe {
            core::arch::asm!(
                // save callee-saved registers
                // https://wiki.osdev.org/System_V_ABI
                "mov rsi, rsp", // save stack before this stack save
                "push rbp",
                "push rbx",
                "push r12",
                "push r13",
                "push r14",
                "push r15",
                "mov rdx, rsp", // *mut SavedRegs argument for unwind
                "call {unwind}",
                "pop r15",
                "pop r14",
                "pop r13",
                "pop r12",
                "pop rbx",
                "pop rbp",
                "ret",
                unwind = sym unwind,
                options(noreturn),
            );
        }
    }

    extern "sysv64" fn unwind(
        _f: *mut &mut dyn FnMut(UnwindRegs),
        stack: u64,
        regs: *mut SavedRegs,
    ) {
        let f = unsafe { &mut *_f };
        let regs = unsafe { &*regs };
        let regs = UnwindRegs {
            r15: regs.r15,
            r14: regs.r14,
            r13: regs.r13,
            r12: regs.r12,
            rbx: regs.rbx,
            rbp: regs.rbp,
            rsp: stack + 8,
            ra: unsafe { *(stack as *const u64) },
        };

        f(regs);
    }
} */

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
            .checked_sub(virt_addr() as u64)
            .and_then(|i| i.checked_add(kernel_base()))
            .unwrap_or(_frame.instr_ptr);
        f(symbol_noerr(instr_ptr));
    }
}

pub fn unwind_stack(f: impl FnMut(FrameInfo)) {
    unsafe { unwind_stack_from(base_ptr(), f) }
}

/// # Safety
///
/// caller must ensure that `ip` points to a valid stack frame
/// and that stackframes end with a NULL
pub unsafe fn print_backtrace_from(ip: VirtAddr) {
    hyperion_log::_print_log_custom(
        LogLevel::Info,
        " BACKTRACE".true_yellow(),
        "begin",
        format_args!("\n"),
    );
    let mut i = 0usize;
    let frame_walker = |FrameInfo {
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
    };
    unsafe { unwind_stack_from(ip, frame_walker) };
    hyperion_log::_print_log_custom(
        LogLevel::Info,
        " BACKTRACE".true_yellow(),
        "end",
        format_args!("\n"),
    );
}

pub fn print_backtrace() {
    unsafe { print_backtrace_from(base_ptr()) }
}

//

static KERNEL_ELF: Lazy<BacktraceResult<ElfBytes<'static, AnyEndian>>> = Lazy::new(|| {
    let bytes = kernel_file().ok_or(BacktraceError::ElfNotLoaded)?;
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
