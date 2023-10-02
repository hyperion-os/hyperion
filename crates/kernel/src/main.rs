#![doc = include_str!("../../../README.md")]
//
#![no_std]
#![no_main]
//
#![feature(
    const_option,
    allocator_api,
    pointer_is_aligned,
    int_roundings,
    array_chunks,
    core_intrinsics,
    custom_test_frameworks,
    panic_can_unwind
)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]

//

use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

use hyperion_arch as arch;
use hyperion_boot as boot;
use hyperion_boot_interface::Cpu;
use hyperion_drivers as drivers;
use hyperion_kernel_info::{NAME, VERSION};
use hyperion_kshell as kshell;
use hyperion_log::*;
use hyperion_log_multi as log_multi;
use hyperion_random as random;
use hyperion_scheduler as scheduler;

extern crate alloc;

//

pub mod panic;
pub mod syscall;
#[cfg(test)]
pub mod testfw;

//

#[no_mangle]
extern "C" fn _start() -> ! {
    // enable logging and and outputs based on the kernel args,
    // any logging before won't be shown
    log_multi::init_logger();

    debug!("Entering kernel_main");
    debug!("{NAME} {VERSION} was booted with {}", boot::NAME);

    //
    arch::syscall::set_handler(syscall::syscall);
    arch::init_bsp_cpu();

    random::provide_entropy(&arch::rng_seed().to_ne_bytes());

    drivers::lazy_install_early();

    #[cfg(test)]
    test_main();

    // main task(s)
    scheduler::executor::spawn(kshell::kshell());

    // jumps to [smp_main] right bellow + wakes up other threads to jump there
    boot::smp_init(smp_main);
}

fn smp_main(cpu: Cpu) -> ! {
    debug!("{cpu} entering smp_main");

    arch::init_smp_cpu(&cpu);

    if cpu.is_boot() {
        drivers::lazy_install_late();
        debug!("boot cpu drivers installed");
    }

    /* scheduler::spawn(move || loop {
        arch::int::enable();
        arch::int::wait();
        arch::int::disable();

        scheduler::yield_now();
    }); */
    scheduler::spawn(move || {
        scheduler::executor::run_tasks();
    });
    debug!("resetting {cpu} scheduler");
    scheduler::reset();
}
