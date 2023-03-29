use super::{args, BOOT_NAME};
use crate::kernel_main;

//

pub use addr::hhdm_offset;
pub use addr::phys_addr;
pub use addr::virt_addr;
pub use cmdline::cmdline;
pub use framebuffer::framebuffer;
pub use mem::memmap;
pub use term::_print;

//

mod addr;
mod cmdline;
mod framebuffer;
mod mem;
mod smp;
mod term;

//

#[no_mangle]
pub extern "C" fn _start() -> ! {
    BOOT_NAME.call_once(|| "Limine");

    args::get().apply();

    kernel_main()
}

pub fn smp_init() {
    smp::init();
}

pub fn boot_cpu() -> crate::smp::Cpu {
    smp::boot_cpu()
}
