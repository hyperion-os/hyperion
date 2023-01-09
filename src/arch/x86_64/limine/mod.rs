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

    // the initial terminal logger crashes if used after initializing GDT and IDT
    crate::log::disable_term();

    gdt::init();
    idt::init();

    x86_64::instructions::interrupts::enable();

    crate::kernel_main()
}

pub fn done() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
