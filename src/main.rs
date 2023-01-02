#![no_std]
#![no_main]
#![feature(format_args_nl)]
#![feature(abi_x86_interrupt)]

use spin::Mutex;

#[path = "arch/x86_64/mod.rs"]
pub mod arch;
pub mod log;
pub mod qemu;
// pub mod vga;

static BOOTLOADER: Mutex<&'static str> = Mutex::new("Hyperion");

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn kernel_main() -> ! {
    println!("Hello from Hyperion");
    println!(" - Hyperion was booted with {}", BOOTLOADER.lock());

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
