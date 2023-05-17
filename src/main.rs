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
#![feature(cfg_target_has_atomic)]
#![feature(slice_as_chunks)]
#![feature(core_intrinsics)]
//
#![feature(custom_test_frameworks)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]

//

use crate::{driver::rtc, scheduler::kshell::kshell, util::fmt::NumberPostfix};

extern crate alloc;

//

#[path = "arch/x86_64/mod.rs"]
pub mod arch;
pub mod boot;
pub mod driver;
pub mod log;
pub mod mem;
pub mod panic;
pub mod scheduler;
pub mod smp;
pub mod term;
#[cfg(test)]
pub mod testfw;
pub mod util;
pub mod vfs;

//

pub static KERNEL_NAME: &str = if cfg!(test) {
    "Hyperion-Testing"
} else {
    "Hyperion"
};

pub static KERNEL_VERSION: &str = env!("CARGO_PKG_VERSION");

// ofc. every kernel has to have this cringy ascii name splash
pub static KERNEL_SPLASH: &str = include_str!("./splash");

pub static KERNEL_BUILD_TIME: &str = env!("HYPERION_BUILD_TIME");

pub static KERNEL_BUILD_REV: &str = env!("HYPERION_BUILD_REV");

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

    info!("\n{KERNEL_SPLASH}");

    if let Some(bl) = boot::BOOT_NAME.get() {
        debug!("{KERNEL_NAME} {KERNEL_VERSION} was booted with {bl}");
    }

    #[cfg(test)]
    test_main();

    if let Some(time) = rtc::RTC.now() {
        debug!("RTC time: {time:?}");
    }

    rtc::RTC.enable_ints();
    rtc::Rtc::install_device();
    debug!(
        "ints enabled?: {}",
        x86_64::instructions::interrupts::are_enabled()
    );

    // main task(s)
    scheduler::spawn(kshell());

    // jumps to [smp_main] right bellow + wakes up other threads to jump there
    smp::init()
}

fn smp_main(cpu: smp::Cpu) -> ! {
    debug!("{cpu} entering smp_main");

    arch::early_per_cpu(&cpu);

    scheduler::run_tasks();
}
