use super::BOOT_NAME;
use crate::{arch, kernel_main};

//

pub use mem::memmap;
pub use term::_print;

//

mod cmdline;
mod framebuffer;
mod mem;
mod smp;
mod term;

//

#[no_mangle]
pub extern "C" fn _start() -> ! {
    BOOT_NAME.call_once(|| "Limine");

    framebuffer::init();
    cmdline::init();

    arch::early_boot_cpu();
    arch::early_per_cpu();

    kernel_main()
}

pub fn smp_init() {
    smp::init();
}
