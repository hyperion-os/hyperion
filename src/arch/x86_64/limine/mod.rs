use super::{gdt, idt};
use crate::debug;

//

pub use term::_print;

//

mod cmdline;
mod framebuffer;
mod term;

//

#[no_mangle]
pub extern "C" fn _start() -> ! {
    crate::BOOTLOADER.call_once(|| "Limine");

    cmdline::init();
    framebuffer::init();

    gdt::init();
    idt::init();

    debug!("Re-enabling x86_64 interrupts");
    x86_64::instructions::interrupts::enable();

    debug!("Calling general kernel_main");
    crate::kernel_main()
}

pub fn done() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
