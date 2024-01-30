#![no_std]

//

pub use addr::{hhdm_offset, phys_addr, size, virt_addr};
pub use cmdline::cmdline;
pub use framebuffer::{framebuffer, init_fb};
pub use kernel::kernel_file;
pub use mem::memmap;
pub use rsdp::rsdp;
pub use smp::{boot_cpu, cpu_count, lapics, smp_init};

//

mod addr;
mod cmdline;
mod framebuffer;
mod kernel;
mod mem;
mod rsdp;
mod smp;

//

pub const NAME: &str = "Limine";
pub const BOOT_STACK_SIZE: u64 = 1 << 16;
