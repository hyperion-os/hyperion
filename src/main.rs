#![doc = include_str!("../README.md")]
//
#![no_std]
#![no_main]
//
#![feature(format_args_nl)]
#![feature(abi_x86_interrupt)]
#![feature(allocator_api)]
#![feature(nonnull_slice_from_raw_parts)]
#![feature(pointer_is_aligned)]
#![feature(int_roundings)]
//
#![feature(custom_test_frameworks)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]

//

use alloc::vec::Vec;
use x86_64::structures::{
    gdt::{self, GlobalDescriptorTable},
    idt::InterruptDescriptorTable,
};

use crate::util::fmt::NumberPostfix;

extern crate alloc;

//

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

pub static KERNEL_VERS: &str = env!("CARGO_PKG_VERSION");

//

// the actual entry exists in [´crate::boot::boot´]
fn kernel_main() -> ! {
    debug!("Entering kernel_main");

    arch::early_boot_cpu();

    x86_64::instructions::interrupts::int3();

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
        debug!("{KERNEL_NAME} {KERNEL_VERS} was booted with {bl}");
    }

    core::hint::black_box((0..128).map(|i| i * 32).collect::<Vec<_>>());

    #[cfg(test)]
    test_main();

    debug!("RNG Seed {}", arch::rng_seed());

    smp::init();
}

fn smp_main(cpu: smp::Cpu) -> ! {
    debug!("{cpu} entering smp_main");

    arch::early_per_cpu(&cpu);

    // x86_64::instructions::interrupts::int3();

    arch::done();
}
