#![no_std]
#![no_main]
#![feature(naked_functions, format_args_nl, generic_nonzero, maybe_uninit_slice)]

//

mod fdt;

//

use core::{
    arch::asm,
    fmt::{self, Arguments},
    panic::PanicInfo,
    str,
};

use spin::{Lazy, Mutex};

//

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        $crate::_print(format_args_nl!($($arg)*))
    };
}

fn _print(args: Arguments) {
    use core::fmt::Write;
    let _ = UART.lock().write_fmt(args);
}

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

    fn init() -> Self {
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

        Self { _p: () }
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

pub static UART: Lazy<Mutex<Uart>> = Lazy::new(|| Mutex::new(Uart::init()));
pub static SYSCON: Mutex<Syscon> = Mutex::new(Syscon::init());

//

extern "C" {
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
    // FIXME: at least try the standard addresses for SYSCON and UART,
    // instead of just panicing after failing to parse the devicetree
    let mut tree = unsafe { fdt::Fdt::read(a1 as _) }.expect("Devicetree is invalid");

    // for entry in tree.iter_reserved_memory() {
    //     println!("{entry:#?}");
    // }

    // tree.structure();

    tree.usable_memory();

    println!("{tree:#x?}");

    println!("size={}", core::mem::size_of::<fdt::Segment>());

    println!("KERNEL_END = {:#x}", unsafe { &_kernel_end } as *const _
        as usize);

    SYSCON.lock().poweroff();

    rust_start();
}

fn rust_start() -> ! {
    println!("Hello, world! (c-a x to exit QEMU / c-c to exit hyperion)");

    loop {
        let Some(c) = UART.lock().read() else {
            continue;
        };

        match c {
            3 => SYSCON.lock().poweroff(),
            4 => SYSCON.lock().reboot(),
            _ => print!("{c}"),
        }
    }

    // halt_and_catch_fire();
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
