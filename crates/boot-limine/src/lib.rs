#![no_std]

//

pub use addr::{hhdm_offset, phys_addr, virt_addr};
pub use cmdline::cmdline;
pub use framebuffer::framebuffer;
use hyperion_boot_interface::kernel_main;
pub use kernel::kernel_file;
pub use mem::{memmap, stack};
pub use rsdp::rsdp;
pub use smp::{boot_cpu, cpu_count, smp_init};
pub use term::_print;

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

pub static NAME: &str = "Limine";

//

#[no_mangle]
extern "C" fn _hyperion_start() -> ! {
    mem::stack_init();

    unsafe { kernel_main() }
}
