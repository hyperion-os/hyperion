#![doc = include_str!("../README.md")]
//
#![no_std]
#![no_main]
//
#![feature(
    format_args_nl,
    abi_x86_interrupt,
    allocator_api,
    pointer_is_aligned,
    int_roundings,
    array_chunks,
    cfg_target_has_atomic,
    slice_as_chunks,
    core_intrinsics,
    custom_test_frameworks,
    panic_can_unwind
)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]

//

use crate::util::fmt::NumberPostfix;

extern crate alloc;

//

#[path = "arch/x86_64/mod.rs"]
pub mod arch;
pub mod backtrace;
pub mod boot;
pub mod driver;
pub mod log;
pub mod mem;
pub mod panic;
pub mod smp;
pub mod term;
#[cfg(test)]
pub mod testfw;
pub mod util;

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

    backtrace::print_backtrace();
    panic!("test panic");

    if let Some(bl) = boot::BOOT_NAME.get() {
        debug!("{KERNEL_NAME} {KERNEL_VERSION} was booted with {bl}");
    }

    #[cfg(test)]
    test_main();

    debug!("RNG Seed {}", arch::rng_seed());

    smp::init()
}

fn smp_main(cpu: smp::Cpu) -> ! {
    debug!("{cpu} entering smp_main");

    arch::early_per_cpu(&cpu);

    debug!("{cpu} halt");
    arch::done();
}
