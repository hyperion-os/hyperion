#![doc = include_str!("../README.md")]
//
#![no_std]
#![no_main]
//
#![feature(format_args_nl)]
#![feature(abi_x86_interrupt)]
#![feature(allocator_api)]
#![feature(pointer_is_aligned)]
#![feature(int_roundings)]
#![feature(array_chunks)]
//
#![feature(custom_test_frameworks)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]

//

use crate::{acpi::apic, util::fmt::NumberPostfix};

extern crate alloc;

//

pub mod acpi;
#[path = "arch/x86_64/mod.rs"]
pub mod arch;
pub mod boot;
pub mod env;
pub mod log;
pub mod mem;
pub mod panic;
pub mod qemu;
pub mod smp;
pub mod term;
#[cfg(test)]
pub mod testfw;
pub mod util;
pub mod video;

//

pub static KERNEL_NAME: &str = if cfg!(test) {
    "Hyperion-Testing"
} else {
    "Hyperion"
};

pub static KERNEL_VERSION: &str = env!("CARGO_PKG_VERSION");

//

// the actual entry exists in [´crate::boot::boot´]
fn kernel_main() -> ! {
    debug!("Entering kernel_main");

    // init the page frame allocator early to make the logs cleaner
    // makes the lazy initialization completely useless btw
    _ = mem::pmm::PageFrameAllocator::get();

    arch::early_boot_cpu();

    debug!("Cmdline: {:?}", boot::args::get());

    debug!(
        "Kernel addr: {:?} ({}B), {:?} ({}B), ",
        boot::virt_addr(),
        boot::virt_addr().as_u64().postfix_binary(),
        boot::phys_addr(),
        boot::phys_addr().as_u64().postfix_binary(),
    );
    debug!("HHDM Offset: {:#0X?}", boot::hhdm_offset());

    // ofc. every kernel has to have this cringy ascii name splash
    info!("\n{}\n", include_str!("./splash"));

    if let Some(bl) = boot::BOOT_NAME.get() {
        debug!("{KERNEL_NAME} {KERNEL_VERSION} was booted with {bl}");
    }

    #[cfg(test)]
    test_main();

    debug!("RNG Seed {}", arch::rng_seed());

    acpi::init();

    smp::init();
}

fn smp_main(cpu: smp::Cpu) -> ! {
    debug!("{cpu} entering smp_main");

    arch::early_per_cpu(&cpu);

    debug!("{cpu} halt");
    arch::done();
}
