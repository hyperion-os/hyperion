use core::{
    arch::asm,
    mem,
    ptr::{self, NonNull},
};

use crate::{arch, boot, println};
use elf::{
    endian::AnyEndian, parse::ParsingTable, string_table::StringTable, symbol::SymbolTable,
    ElfBytes, ParseError,
};
use spin::Lazy;

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

pub fn symbol(instr_ptr: u64) -> Result<&'static str, BacktraceError> {
    let (symtab, strtab) = SYMTAB.as_ref().map_err(BacktraceError::Inner)?;

    let symbol = symtab
        .iter()
        .find(|sym| (sym.st_value..sym.st_value + sym.st_size).contains(&instr_ptr));

    let Some(symbol) = symbol else {
        return Ok(UNKNOWN);
    };

    strtab
        .get(symbol.st_name as _)
        .map_err(BacktraceError::ElfParse)
}

pub fn unwind_stack(mut f: impl FnMut(usize, &'static str)) {
    arch::int::disable();

    // TODO: move to arch
    let mut frame_ptr: usize;
    let mut instr_ptr: usize = x86_64::registers::read_rip().as_u64() as _;
    unsafe {
        asm!("mov {}, rbp", out(reg) frame_ptr);
    }

    println!("{frame_ptr} {instr_ptr}");

    if frame_ptr == 0 {
        println!("empty");
    }

    loop {
        if frame_ptr == 0 {
            break;
        }

        let rip_rbp = frame_ptr + mem::size_of::<usize>();

        let instr_ptr = unsafe { ptr::read_volatile(rip_rbp as *const usize) };
        if instr_ptr == 0 {
            break;
        }

        frame_ptr = unsafe { ptr::read_volatile(frame_ptr as *const usize) };

        f(instr_ptr, symbol(instr_ptr as _).unwrap_or(UNKNOWN));
    }

    // TODO: should reset to what it was before
    arch::int::enable();
}

pub fn print_backtrace() {
    println!("--[ begin backtrace ]--");
    let mut i = 0usize;
    unwind_stack(|ip, sym| {
        println!("{i:>3} : {ip:#018x} - {sym}");
        i += 1;
    });
    println!("--[ end backtrace ]--");
}
