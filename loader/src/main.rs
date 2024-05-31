#![no_std]
#![no_main]
#![feature(naked_functions, format_args_nl, generic_nonzero, maybe_uninit_slice)]

//! hyperion kernel loader for RISC-V
//!
//! this loads the baked in kernel ELF into higher half virtual memory, and then jumps there

//

mod fdt;
mod logger;

//

use core::{
    arch::asm,
    fmt::{self},
    mem::MaybeUninit,
    num::NonZero,
    panic::PanicInfo,
    ptr, str,
};

use log::{print, println};
use spin::{Lazy, Mutex};
use util::rle::Region;
use xmas_elf::ElfFile;

//

pub struct Uart {
    _p: (),
}

impl Uart {
    pub fn write(&mut self, byte: u8) {
        unsafe { Self::base().write_volatile(byte) };
    }

    pub fn read(&mut self) -> Option<u8> {
        let base = Self::base();

        // anything to read? <- LSR line status
        let avail = unsafe { base.add(5).read_volatile() } & 0b1 != 0;
        // let avail = false;

        if avail {
            Some(unsafe { base.read_volatile() })
        } else {
            None
        }
    }

    const fn new() -> Self {
        Self { _p: () }
    }

    fn init(&mut self) {
        let base = Self::base();

        unsafe {
            // data size to 2^0b11=2^3=8 bits -> IER interrupt enable
            base.add(3).write_volatile(0b11);
            // enable FIFO                    -> FCR FIFO control
            base.add(2).write_volatile(0b1);
            // enable interrupts              -> LCR line control
            base.add(1).write_volatile(0b1);

            // TODO (HARDWARE): real UART
        }
    }

    const fn base() -> *mut u8 {
        0x1000_0000 as _
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write(byte);
        }
        Ok(())
    }
}

//

pub struct Syscon {
    _p: (),
}

impl Syscon {
    pub fn poweroff(&mut self) -> ! {
        unsafe { Self::base().write_volatile(0x5555) };
        halt_and_catch_fire();
    }

    pub fn reboot(&mut self) -> ! {
        unsafe { Self::base().write_volatile(0x7777) };
        halt_and_catch_fire();
    }

    const fn init() -> Self {
        Self { _p: () }
    }

    const fn base() -> *mut u32 {
        0x10_0000 as _
    }
}

//

// pub static UART: Lazy<Mutex<Uart>> = Lazy::new(|| Mutex::new(Uart::init()));
pub static SYSCON: Mutex<Syscon> = Mutex::new(Syscon::init());

//

extern "C" {
    static _kernel_beg: ();
    static _kernel_end: ();
}

#[no_mangle]
#[link_section = ".text.boot"]
unsafe extern "C" fn _start() -> ! {
    unsafe {
        asm!(
            // init global pointer
            ".option push",
            ".option norelax",
            "la gp, _global_pointer",
            ".option pop",

            // a0 == 0x80200000 (but isn't for some reason?)
            // a1 == DTB (device tree binary)

            // init stack
            "la sp, _boot_stack_top",

            // call rust code
            "tail {entry}",
            entry = sym entry,
            options(noreturn)
        );
    }
}

extern "C" fn entry(_a0: usize, a1: usize) -> ! {
    logger::init_logger();

    // FIXME: at least try the standard addresses for SYSCON and UART,
    // instead of just panicing after failing to parse the devicetree
    let mut tree = unsafe { fdt::Fdt::read(a1 as _) }.expect("Devicetree is invalid");

    let mut memory = tree.usable_memory();

    // reserve the kernel memory from the usable memory
    let kernel_beg = unsafe { &_kernel_beg } as *const _ as usize;
    let kernel_end = unsafe { &_kernel_end } as *const _ as usize;
    memory.remove(Region {
        addr: kernel_beg,
        size: NonZero::new(kernel_end - kernel_beg).unwrap(),
    });

    println!("{memory:#x?}");

    // // test fill whole memory space with zeros
    // for usable in memory.iter_usable() {
    //     let region: *mut [MaybeUninit<u64>] =
    //         ptr::slice_from_raw_parts_mut(usable.addr as _, usable.size.get() / 8);
    //     let region = unsafe { &mut *region };

    //     for p in region {
    //         // println!("write {:x}", p as *const _ as usize);
    //         p.write(0);
    //     }
    // }

    // mem::init_frame_allocator(memory);

    // let x = unsafe { &_kernel_end } as *const () as *mut u8;
    // for i in 0.. {
    //     println!("probing {:#x}", x as usize + i * 0x1000);
    //     unsafe { x.add(i * 0x1000).write(5) };
    // }

    static KERNEL_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL"));
    let kernel_elf = ElfFile::new(&KERNEL_ELF).expect("invalid ELF");

    for program in kernel_elf.program_iter() {
        println!("load {program:?}");
        if let Some(xmas_elf::program::Type::Load) = program.get_type() {
            let data = program.get_data(&kernel_elf);

            program.virtual_addr();
            program.align();
        }
    }

    let entry = kernel_elf.header.pt2.entry_point();

    println!("entry={entry:#x}");

    println!("done, poweroff");
    SYSCON.lock().poweroff();
}

/// HCF instruction
fn halt_and_catch_fire() -> ! {
    loop {
        wait_for_interrupts();
    }
}

/// WFI instruction
extern "C" fn wait_for_interrupts() {
    unsafe {
        asm!("wfi");
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{info}");
    SYSCON.lock().poweroff();
}
