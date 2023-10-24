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
    panic_can_unwind,
    type_name_of_val
)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(clippy::needless_return)]

//

use alloc::{format, vec::Vec};
use core::{
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperion_arch as arch;
use hyperion_boot as boot;
use hyperion_boot_interface::Cpu;
use hyperion_drivers as drivers;
use hyperion_kernel_info::{NAME, VERSION};
use hyperion_kshell as kshell;
use hyperion_log::*;
use hyperion_log_multi as log_multi;
use hyperion_mem::from_higher_half;
use hyperion_random as random;
use hyperion_scheduler as scheduler;
use spin::Once;
use x86_64::VirtAddr;

extern crate alloc;

//

pub mod panic;
pub mod syscall;
#[cfg(test)]
pub mod testfw;

//

static BSP_BOOT_STACK: Once<Range<VirtAddr>> = Once::new();

//

#[no_mangle]
extern "C" fn _start() -> ! {
    let boot_stack = arch::stack_pages();
    BSP_BOOT_STACK.call_once(move || boot_stack);

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

    scheduler::schedule(move || {
        scheduler::rename("<spammer>".into());
        static INC: AtomicUsize = AtomicUsize::new(0);

        let pid = scheduler::lock_active().info().pid;
        info!("I am pid:{pid}");
        loop {
            // broadcast b"hello n" to every process running
            for task in scheduler::tasks() {
                scheduler::send(
                    task.pid,
                    Vec::from(format!("hello {}", INC.fetch_add(1, Ordering::SeqCst))).into(),
                );
            }
            scheduler::recv(); // this also sends to itself so this never blocks

            // wait 200ms
            scheduler::sleep(time::Duration::milliseconds(200));
        }
    });
    scheduler::schedule(move || {
        scheduler::rename("<reader>".into());

        let pid = scheduler::lock_active().info().pid;
        info!("I am pid:{pid}");
        loop {
            // block on recv and print out the result
            info!("got {:?}", core::str::from_utf8(&scheduler::recv()));
        }
    });

    // jumps to [smp_main] right bellow + wakes up other threads to jump there
    boot::smp_init(smp_main);
}

fn smp_main(cpu: Cpu) -> ! {
    let mut boot_stack = arch::stack_pages();

    trace!("{cpu} entering smp_main");

    arch::init_smp_cpu(&cpu);

    if cpu.is_boot() {
        boot_stack = BSP_BOOT_STACK
            .get()
            .expect("_start to run before smp_main")
            .clone();

        drivers::lazy_install_late();
        debug!("boot cpu drivers installed");
    }

    debug!("boot stack: {boot_stack:?}");

    /* scheduler::spawn(move || {
        scheduler::rename("<loop>".into());
        scheduler::spawn(move || {
            scheduler::rename("<loop>".into());
        });
        loop {
            scheduler::yield_now();
        }
    });
    scheduler::spawn(move || {
        scheduler::rename("<loop>".into());
        scheduler::spawn(move || {
            scheduler::rename("<loop>".into());
        });
        loop {
            scheduler::yield_now();
        }
    }); */

    scheduler::schedule(move || {
        scheduler::rename("<kernel futures executor>".into());

        let first = from_higher_half(boot_stack.start);
        let count = ((boot_stack.end - boot_stack.start) / 0x1000) as usize;

        let frames = unsafe { hyperion_mem::pmm::PageFrame::new(first, count) };
        debug!("deallocating bootloader provided stack");
        hyperion_mem::pmm::PFA.free(frames);

        scheduler::executor::run_tasks();
    });
    trace!("resetting {cpu} scheduler");
    scheduler::init();
}
