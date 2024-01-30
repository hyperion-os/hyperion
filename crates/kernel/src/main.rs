#![doc = include_str!("../../../README.md")]
//
#![no_std]
#![no_main]
//
#![allow(internal_features)]
#![feature(custom_test_frameworks, lang_items)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(clippy::needless_return)]

//

extern crate alloc;

use arch::vmm::PageMap;
use boot::BOOT_STACK_SIZE;
use hyperion_arch as arch;
use hyperion_boot as boot;
use hyperion_cpu_id::cpu_id;
use hyperion_drivers as drivers;
use hyperion_futures as futures;
use hyperion_kernel_impl::VFS_ROOT;
use hyperion_kernel_info::{NAME, VERSION};
use hyperion_log::*;
use hyperion_log_multi as log_multi;
use hyperion_mem::{from_higher_half, vmm::PageMapImpl, KernelSlabAlloc};
use hyperion_random as random;
use hyperion_scheduler as scheduler;
use hyperion_sync as sync;
use x86_64::VirtAddr;

//

pub mod panic;
pub mod syscall;
#[cfg(test)]
pub mod testfw;

//

#[global_allocator]
static ALLOCATOR: KernelSlabAlloc<spin::Mutex<()>> = KernelSlabAlloc::new();

//

#[no_mangle]
extern "sysv64" fn rust_start(sp: usize) -> ! {
    if sync::once!() {
        // enable logging and and outputs based on the kernel args,
        // any logging before won't be shown
        log_multi::init_logger();

        debug!("Entering kernel_main");
        debug!("{NAME} {VERSION} was booted with {}", boot::NAME);

        // user-space syscall handler
        arch::syscall::set_handler(syscall::syscall);
    }

    // init GDT, IDT, TSS, TLS and cpu_id
    arch::init();

    // wake up all cpus
    arch::wake_cpus();

    let boot_vmm = PageMap::current();

    // init task per cpu
    debug!("init CPU-{}", cpu_id());
    scheduler::init(move || init(sp, boot_vmm));
}

fn init(boot_sp: usize, mut boot_vmm: PageMap) {
    scheduler::rename("<kernel async>");

    // init task once
    if sync::once!() {
        // random hw specifics
        random::provide_entropy(&arch::rng_seed().to_ne_bytes());
        drivers::lazy_install_early(VFS_ROOT.clone());
        drivers::lazy_install_late();

        // os unit tests
        #[cfg(test)]
        test_main();
        // kshell (kernel-space shell) UI task(s)
        #[cfg(not(test))]
        futures::spawn(hyperion_kshell::kshell());
    }

    // The bootloader provided vmm is shared between CPUs
    // so this makes sure that only the last processor still using it,
    // is the only one that can delete it.
    if sync::last!() {
        debug!(
            "freeing bootloader page table: {:#x}",
            boot_vmm.cr3().start_address()
        );
        unsafe {
            boot_vmm.mark_owned();
        };
        drop(boot_vmm);
    }

    let first = from_higher_half(VirtAddr::new(boot_sp as _));
    let count = (BOOT_STACK_SIZE >> 12) as usize;

    // Bootloader provided stack can be freed after switching away from
    // the bootloader task.
    let frames = unsafe { hyperion_mem::pmm::PageFrame::new(first, count) };
    trace!("deallocating bootloader provided stack {boot_sp:#018x} (size:{BOOT_STACK_SIZE})");
    hyperion_mem::pmm::PFA.free(frames);

    // start doing kernel things
    futures::run_tasks();
}

// to fix `cargo clippy` without a target
#[cfg(any(feature = "cargo-clippy", not(target_os = "none")))]
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
