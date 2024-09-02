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

use core::slice;

use hyperion_arch::{self as arch, vmm::PageMap};
use hyperion_boot as boot;
use hyperion_cpu_id::cpu_id;
use hyperion_kernel_info::{NAME, VERSION};
use hyperion_log::*;
use hyperion_log_multi as log_multi;
use hyperion_mem::{
    pmm,
    vmm::{MapTarget, PageMapImpl},
};
use hyperion_scheduler as scheduler;
use hyperion_sync as sync;
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

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

        // user-space syscall handler
        arch::syscall::set_handler(syscall::syscall);
    }

    // init GDT, IDT, TSS, TLS and cpu_id
    arch::init();

    // wake up all cpus
    arch::wake_cpus(_start);

    if sync::once!() {
        let vm = PageMap::new();
        println!("new page map");

        // map the bootstrap binary to 0x8000_0000
        let bootstrap = include_bytes!(env!("BOOTSTRAP_BIN"));
        let bootstrap_bin_mem = pmm::PFA.alloc(bootstrap.len().div_ceil(0x1000));
        let target = MapTarget::Preallocated(bootstrap_bin_mem.physical_addr());
        let base = VirtAddr::new(0x8000_0000);
        vm.map(
            base..base + bootstrap_bin_mem.byte_len(),
            target,
            PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE,
        );
        hyperion_kernel_impl::phys_write_into_proc(&vm, 0x8000_0000, &bootstrap[..]).unwrap();
        assert!(bootstrap_bin_mem.byte_len() <= 0x8000_0000);

        // map its stack to below 0x8000_0000
        let stack_mem = pmm::PFA.alloc(16);
        let target = MapTarget::Preallocated(bootstrap_bin_mem.physical_addr());
        vm.map(
            base - stack_mem.byte_len()..base,
            target,
            PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
        );

        // map initfs blob to 0xF000_0000
        let initfs = boot::modules()
            .find_map(|module| {
                (module.cmdline == Some("initfs"))
                    .then_some((VirtAddr::new(module.addr as _), module.size))
            })
            .expect("no initfs");
        let initfs_bytes = unsafe { slice::from_raw_parts(initfs.0.as_ptr(), initfs.1) };
        let initfs_mem = pmm::PFA.alloc(initfs_bytes.len().div_ceil(0x1000));
        let target = MapTarget::Preallocated(initfs_mem.physical_addr());
        let base = VirtAddr::new(0xF000_0000);
        vm.map(
            base..base + initfs_mem.byte_len(),
            target,
            PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
        );
        hyperion_kernel_impl::phys_write_into_proc(&vm, 0xF000_0000, initfs_bytes).unwrap();

        // map some lazy allocated heap memory to 0x8_0000_0000
        let base = VirtAddr::new(0x8_0000_0000);
        vm.map(
            base..base + 0x8_0000_0000usize,
            MapTarget::LazyAlloc,
            PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
        );

        scheduler::init_bootstrap(vm.cr3(), 0x8000_0000, 0x8000_0000);
        core::mem::forget(vm);
    }

    // init task per cpu
    debug!("init CPU-{}", cpu_id());
    scheduler::done();
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
