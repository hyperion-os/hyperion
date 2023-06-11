pub use addr::{hhdm_offset, phys_addr, virt_addr};
pub use cmdline::cmdline;
pub use framebuffer::framebuffer;
use hyperion_boot_interface::{
    framebuffer::FramebufferCreateInfo,
    loader::{provide_boot, Bootloader},
    smp::Cpu,
};
pub use kernel::kernel_file;
pub use mem::{memmap, stack};
pub use rsdp::rsdp;
pub use smp::{boot_cpu, init as smp_init};
pub use term::_print;

use self::smp::cpu_count;
use super::args;
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
    provide_boot(&LimineBoot);
    args::get().apply();

    kernel_main()
}

struct LimineBoot;

impl Bootloader for LimineBoot {
    fn name(&self) -> &'static str {
        "Limine"
    }

    fn framebuffer(&self) -> Option<FramebufferCreateInfo> {
        framebuffer()
    }

    fn hhdm_offset(&self) -> u64 {
        hhdm_offset()
    }

    fn rsdp(&self) -> Option<*const ()> {
        Some(rsdp())
    }

    fn bsp(&self) -> Cpu {
        smp::boot_cpu()
    }

    fn smp_init(&self, dest: fn(Cpu) -> !) -> ! {
        smp_init(dest)
    }

    fn cpu_count(&self) -> usize {
        cpu_count()
    }
}
