pub use addr::{hhdm_offset, phys_addr, virt_addr};
pub use cmdline::cmdline;
pub use framebuffer::framebuffer;
pub use kernel::kernel_file;
pub use mem::{memmap, stack};
pub use rsdp::rsdp;
pub use smp::{boot_cpu, init as smp_init};
pub use term::_print;

use super::{args, BOOT_NAME};
use crate::kernel_main;

//

mod addr;
mod cmdline;
mod framebuffer;
mod kernel;
mod mem;
mod rsdp;
mod smp;
mod term;

//

#[no_mangle]
pub extern "C" fn _start() -> ! {
    mem::stack_init();
    BOOT_NAME.call_once(|| "Limine");
    args::get().apply();

    kernel_main()
}
