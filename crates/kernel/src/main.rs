#![doc = include_str!("../../../README.md")]
//
#![no_std]
#![no_main]
//
#![allow(internal_features)]
#![feature(custom_test_frameworks, lang_items, naked_functions, try_blocks)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(clippy::needless_return)]

//

extern crate alloc;

use hyperion_arch::{self as arch, generate_handler};
use hyperion_boot as boot;
use hyperion_cpu_id::cpu_id;
// use hyperion_drivers as drivers;
use hyperion_futures as futures;
// use hyperion_kernel_impl::VFS_ROOT;
use hyperion_kernel_info::{NAME, VERSION};
use hyperion_loader::Loader;
use hyperion_log as log;
use hyperion_log::*;
use hyperion_log_multi as log_multi;
use hyperion_random as random;
use hyperion_scheduler as scheduler;
use hyperion_sync as sync;

//

pub mod panic;
pub mod syscall;
#[cfg(test)]
pub mod testfw;

//

#[no_mangle]
extern "C" fn _start() -> ! {
    hyperion_boot::init_fb();

    if sync::once!() {
        // enable logging and and outputs based on the kernel args,
        // any logging before won't be shown
        log_multi::init_logger();

        debug!("Entering kernel_main");
        debug!("{NAME} {VERSION} was booted with {}", boot::NAME);
    }

    // init GDT, IDT, TSS, TLS and cpu_id
    arch::init(generate_handler!(syscall::syscall));

    // wake up all cpus
    arch::wake_cpus(_start);

    // init ACPI
    hyperion_driver_acpi::init();

    // init task once
    if sync::once!() {
        // random hw specifics
        random::provide_entropy(&arch::rng_seed().to_ne_bytes());
        // drivers::lazy_install_early(VFS_ROOT.clone());
        // drivers::lazy_install_late();
        hyperion_clock::set_source_picker(|| Some(&*hyperion_driver_acpi::hpet::HPET));

        // os unit tests
        #[cfg(test)]
        test_main();
    }

    if sync::once!() {
        futures::spawn(async move {
            use hyperion_mem::vmm::PageMapImpl;
            let proc = scheduler::proc::Process::new();
            // proc.address_space.activate();
            let tmptask = scheduler::task::RunnableTask::new_in(0, 0, proc.clone());
            tmptask.set_active();

            let bin = include_bytes!(env!("CARGO_BIN_FILE_SAMPLE_ELF"));
            let mut loader = Loader::new(bin, proc).unwrap();

            loader.load();
            loader.finish().unwrap().ready();
        });
        futures::spawn(async move {
            hyperion_log::debug!("running tick task");

            loop {
                hyperion_log::debug!("tick (CPU-{})", cpu_id());
                futures::timer::sleep(time::Duration::milliseconds(1000)).await;
            }
        });
    }

    // init scheduling
    debug!("init CPU-{}", cpu_id());
    scheduler::init();
    hyperion_syscall::exit(0); // use a syscall from kernel space to enter the main loop
}

// to fix `cargo clippy` without a target
#[cfg(any(clippy, not(target_os = "none")))]
#[lang = "eh_personality"]
fn eh_personality() {}

//

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;

    use hyperion_scheduler as scheduler;
    use scheduler::{ipc::pipe::Pipe, lock::Mutex, spawn, yield_now};

    #[test_case]
    fn scheduler_pipe() {
        let (pipe_tx, pipe_rx) = Pipe::new_pipe().split();

        spawn(move || {
            pipe_tx.send_slice(b"some random bytes").unwrap();
        });

        let mut buf = [0u8; 64];
        let len = pipe_rx.recv_slice(&mut buf).unwrap();
        assert_eq!(&buf[..len], b"some random bytes")
    }

    #[test_case]
    fn scheduler_mutex_trivial() {
        let mutex = Mutex::new(5);

        assert_eq!(*mutex.lock(), 5);

        *mutex.lock() = 10;

        assert_eq!(*mutex.lock(), 10);
    }

    #[test_case]
    fn scheduler_mutex_multithread() {
        let mutex = Arc::new(Mutex::new(5));

        for _ in 0..3 {
            let mutex = mutex.clone();
            scheduler::spawn(move || {
                *mutex.lock() += 1;
            });
        }

        loop {
            if *mutex.lock() == 8 {
                break;
            }

            yield_now();
        }
    }
}
