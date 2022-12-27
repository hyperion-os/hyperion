#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

pub mod cfg;
pub mod framebuffer;
pub mod idt;
pub mod instructions;

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    instructions::hlt();
}

fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    framebuffer::init(boot_info);
    framebuffer::clear();
    framebuffer::print_char(b'H');

    idt::init();

    instructions::hlt();
}
