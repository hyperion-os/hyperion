#![doc = include_str!("../README.md")]
//
#![no_std]
#![no_main]
//
#![feature(
    const_option,
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

use core::sync::atomic::{AtomicUsize, Ordering};

use chrono::Duration;
use futures_util::StreamExt;
use x86_64::VirtAddr;

use self::{
    arch::rng_seed,
    driver::{
        acpi::hpet::HPET,
        video::{color::Color, framebuffer::Framebuffer},
    },
    scheduler::timer::sleep,
};
use crate::{
    arch::cpu::idt::Irq,
    driver::acpi::ioapic::IoApic,
    mem::from_higher_half,
    scheduler::{kshell::kshell, tick::Ticks},
    smp::CPU_COUNT,
    util::fmt::NumberPostfix,
};

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
    debug!(
        "Kernel Stack: {:#0X?}",
        from_higher_half(VirtAddr::new(boot::stack().start as u64))
    );

    if let Some(bl) = boot::BOOT_NAME.get() {
        debug!("{KERNEL_NAME} {KERNEL_VERSION} was booted with {bl}");
    }

    #[cfg(test)]
    test_main();

    // main task(s)
    scheduler::spawn(kshell());
    scheduler::spawn(spinner());

    // jumps to [smp_main] right bellow + wakes up other threads to jump there
    smp::init()
}

fn smp_main(cpu: smp::Cpu) -> ! {
    debug!("{cpu} entering smp_main");

    arch::early_per_cpu(&cpu);

    spin::Lazy::force(&HPET);

    static CPU_COUNT_AFTER_INIT: AtomicUsize = AtomicUsize::new(0);
    if Some(CPU_COUNT_AFTER_INIT.fetch_add(1, Ordering::SeqCst) + 1) == CPU_COUNT.get().copied() {
        // code after every CPU and APIC has been initialized
        if let Some(mut io_apic) = IoApic::any() {
            io_apic.set_irq_any(1, Irq::PicKeyboard as _);
            debug!("keyboard initialized");
        }
    }

    scheduler::run_tasks();
}

async fn spinner() {
    loop {
        sleep(Duration::milliseconds(100)).await;
        let Some(mut fbo) = Framebuffer::get_manual_flush() else {
            crate::warn!("failed to get fbo");
            break;
        };

        let r = (rng_seed() % 0xFF) as u8;
        let g = (rng_seed() % 0xFF) as u8;
        let b = (rng_seed() % 0xFF) as u8;
        let x = fbo.width - 60;
        let y = fbo.height - 60;
        fbo.fill(x, y, 50, 50, Color::new(r, g, b));
    }
}
