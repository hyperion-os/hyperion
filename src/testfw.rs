//! Tests should only be ran on a single thread at the moment

use crate::{log, print, println};
use core::{
    any::type_name,
    panic::PanicInfo,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use spin::Once;
use x86_64::instructions::port::Port;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub trait TestCase: Sync {
    fn run(&self);
}

//

impl<F: Fn() + Sync> TestCase for F {
    fn run(&self) {
        let name = type_name::<Self>();
        print!(" - {name:.<60}");
        self();
    }
}

//

pub fn exit_qemu(exit_code: QemuExitCode) {
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

pub fn test_runner(tests: &'static [&'static dyn TestCase]) {
    TESTS.call_once(|| tests);

    log::set_log_level(log::LogLevel::None);
    println!("Running {} tests", tests.len());
    run_tests();

    exit_qemu(QemuExitCode::Success);
}

pub fn next_test() -> Option<&'static dyn TestCase> {
    TESTS
        .get()
        .and_then(|tests| tests.get(IDX.fetch_add(1, Ordering::SeqCst)))
        .copied()
}

pub fn run_tests() {
    while let Some(next_test) = next_test() {
        NEXT_SHOULD_PANIC.store(false, Ordering::SeqCst);

        next_test.run();

        verify_outcome(None);
    }
}

pub fn test_panic_handler(info: &PanicInfo) {
    verify_outcome(Some(info));

    // a hack to keep running tests even tho a panic happened
    run_tests();

    if SUCCESSFUL.load(Ordering::SeqCst) {
        exit_qemu(QemuExitCode::Success);
    } else {
        exit_qemu(QemuExitCode::Failed);
    }
}

pub fn verify_outcome(panic_info: Option<&PanicInfo>) {
    if NEXT_SHOULD_PANIC.load(Ordering::SeqCst) == panic_info.is_some() {
        println!("[ok]");
    } else {
        if let Some(panic_info) = panic_info {
            println!("[failed]\n{panic_info}");
        } else {
            println!("[failed]");
        }
        SUCCESSFUL.store(false, Ordering::SeqCst);
    }
}

/// NOTE: Every panic cannot be handled
///
/// Double faults and page faults for example cannot be handled
pub fn should_panic() {
    NEXT_SHOULD_PANIC.store(true, Ordering::SeqCst);
}

static TESTS: Once<&'static [&'static dyn TestCase]> = Once::new();
static IDX: AtomicUsize = AtomicUsize::new(0);

// TODO: thread local
static NEXT_SHOULD_PANIC: AtomicBool = AtomicBool::new(false);
// TODO: thread local
static SUCCESSFUL: AtomicBool = AtomicBool::new(true);

#[cfg(test)]
mod tests {
    /* use crate::{debug, println}; */

    #[allow(clippy::eq_op)]
    #[test_case]
    fn trivial() {
        assert_eq!(0, 0);
    }

    #[test_case]
    fn should_panic() {
        // mark this test to be a panic=success
        super::should_panic();
        assert_eq!(0, 1);
    }
}
