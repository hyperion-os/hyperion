#![no_std]
#![no_main]
#![feature(format_args_nl)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![feature(type_alias_impl_trait)]
#![feature(result_option_inspect)]
#![feature(allocator_api)]
#![feature(nonnull_slice_from_raw_parts)]
#![feature(exclusive_range_pattern)]
#![feature(let_chains)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![doc = include_str!("../README.md")]

//

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
    debug!("Cmdline: {:?}", boot::args::get());

    debug!(
        "Kernel addr: {:?}, {:?} ({}B), HHDM Offset: {:#0X?}",
        boot::virt_addr(),
        boot::phys_addr(),
        boot::phys_addr().as_u64().postfix_binary(),
        boot::hhdm_offset()
    );

    debug_phys_addr!(boot::phys_addr());
    debug_phys_addr!(x86_64::PhysAddr(boot::hhdm_offset()));

    debug!(
        "{:?}",
        (0u32..32)
            .map(|i| 2u32.pow(i))
            .collect::<alloc::vec::Vec<_>>()
    );

    // ofc. every kernel has to have this cringy ascii name splash
    info!("\n{}\n", include_str!("./splash"));

    if let Some(bl) = boot::BOOT_NAME.get() {
        debug!("{KERNEL_NAME} {KERNEL_VERS} was booted with {bl}");
    }

    #[cfg(test)]
    test_main();

    debug!("RNG Seed {}", arch::rng_seed());

    smp::init();
}

fn smp_main(cpu: smp::Cpu) -> ! {
    debug!("{cpu} entering smp_main");

    // x86_64::instructions::interrupts::int3();

    arch::done();
}
