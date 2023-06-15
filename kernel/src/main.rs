#![doc = include_str!("../../README.md")]
//
#![no_std]
#![no_main]
//
#![feature(
    const_option,
    abi_x86_interrupt,
    allocator_api,
    pointer_is_aligned,
    int_roundings,
    array_chunks,
    core_intrinsics,
    custom_test_frameworks,
    panic_can_unwind,
    lang_items
)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]

//

use chrono::Duration;
use futures_util::StreamExt;
use hyperion_boot_interface::Cpu;
use hyperion_color::Color;
use hyperion_framebuffer::framebuffer::Framebuffer;
use hyperion_kernel_info::{NAME, VERSION};
use hyperion_log::{debug, warn};
use hyperion_scheduler::timer::ticks;

use self::arch::rng_seed;

extern crate alloc;

//

#[path = "./arch/x86_64/mod.rs"]
pub mod arch;
pub mod backtrace;
pub mod panic;
#[cfg(test)]
pub mod testfw;

//

#[no_mangle]
fn kernel_main() -> ! {
    // enable logging and and outputs based on the kernel args,
    // any logging before won't be shown
    hyperion_log_multi::init_logger();

    debug!("Entering kernel_main");
    debug!("{NAME} {VERSION} was booted with {}", hyperion_boot::NAME);

    arch::early_boot_cpu();

    hyperion_drivers::lazy_install_early();

    #[cfg(test)]
    test_main();

    // main task(s)
    hyperion_scheduler::spawn(hyperion_kshell::kshell());
    hyperion_scheduler::spawn(spinner());

    // jumps to [smp_main] right bellow + wakes up other threads to jump there
    hyperion_boot::smp_init(smp_main);
}

fn smp_main(cpu: Cpu) -> ! {
    debug!("{cpu} entering smp_main");

    arch::early_per_cpu(&cpu);

    if cpu.is_boot() {
        hyperion_drivers::lazy_install_late();
    }

    hyperion_scheduler::run_tasks();
}

async fn spinner() {
    let mut ticks = ticks(Duration::milliseconds(500));

    while ticks.next().await.is_some() {
        let Some(fbo) = Framebuffer::get() else {
            warn!("failed to get fbo");
            break;
        };
        let mut fbo = fbo.lock();

        let r = (rng_seed() % 0xFF) as u8;
        let g = (rng_seed() % 0xFF) as u8;
        let b = (rng_seed() % 0xFF) as u8;
        let x = fbo.width - 60;
        let y = fbo.height - 60;
        fbo.fill(x, y, 50, 50, Color::new(r, g, b));
    }
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
