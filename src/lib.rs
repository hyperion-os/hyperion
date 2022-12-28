#![no_std]
#![no_main]

#[path = "arch/x86_64/mod.rs"]
pub mod arch;

pub mod vga;

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn kernel_main() -> ! {
    // null byte clears the VGA buffer
    // print!("\0");

    // println!("Hello from Hyperion, pointer = {pointer:#x}, fb = {fb:#x}");

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
