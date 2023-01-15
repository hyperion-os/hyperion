use crate::println;

use super::{gdt, idt};

//

pub use term::_print;

//

mod framebuffer;
mod term;

//

#[no_mangle]
pub extern "C" fn _start() -> ! {
    x86_64::instructions::interrupts::disable();
    *crate::BOOTLOADER.lock() = "Limine";

    framebuffer::init();

    gdt::init();
    idt::init();

    println!("Re-enabling x86_64 interrupts");
    x86_64::instructions::interrupts::enable();

    println!("Calling general kernel_main");
    crate::kernel_main()
}

pub fn done() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
