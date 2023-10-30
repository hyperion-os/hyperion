//! Tests should only be ran on a single thread at the moment

use alloc::{format, string::String, vec::Vec};
use core::{
    any::type_name,
    panic::PanicInfo,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use crossbeam::queue::SegQueue;
use hyperion_log::{error, print, println, LogLevel};
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

    fn should_panic(&self) -> bool {
        self.name().ends_with("_should_panic")
    }

    fn name(&self) -> &'static str;
}

//

impl<F: Fn() + Sync> TestCase for F {
    fn run(&self) {
        print!(" - {:.<60}", self.name());
        self();
    }

    fn name(&self) -> &'static str {
        type_name::<Self>()
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
    hyperion_log_multi::set_fbo(LogLevel::None);
    hyperion_log_multi::set_qemu(LogLevel::None);

    println!("Running {} tests", tests.len());
    // run_tests();

    for (i, test) in tests.iter().enumerate() {
        hyperion_scheduler::schedule(move || {
            let name = test.name();
            hyperion_scheduler::rename(name.into());

            test.run();

            let should_panic = name.ends_with("should_panic");
            if should_panic {
                TESTS_FAILS.push(format!("`{name}` was expected to panic but it didn't"));
                println!("[err]")
            } else {
                println!("[ok]")
            }
            TESTS_COMPLETE.fetch_add(1, Ordering::SeqCst);
        });
    }

    hyperion_scheduler::schedule(move || loop {
        let completed = TESTS_COMPLETE.load(Ordering::SeqCst);
        // println!("completed: {completed}");
        if completed != tests.len() {
            hyperion_scheduler::yield_now_wait();
            continue;
        }

        let mut fails = false;
        while let Some(err) = TESTS_FAILS.pop() {
            error!("ERROR: {err}");
        }

        if fails {
            exit_qemu(QemuExitCode::Failed)
        } else {
            exit_qemu(QemuExitCode::Success)
        }
    });

    hyperion_scheduler::init();
}

static TESTS_COMPLETE: AtomicUsize = AtomicUsize::new(0);
static TESTS_FAILS: SegQueue<String> = SegQueue::new();

/* pub fn next_test() -> Option<&'static dyn TestCase> {
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
} */

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    let name = hyperion_scheduler::task().name.read().clone();
    let should_panic = name.ends_with("should_panic");

    if !should_panic {
        TESTS_FAILS.push(format!("`{name}` paniced unexpectedly"));
        println!("[err]")
    } else {
        println!("[ok]")
    }
    TESTS_COMPLETE.fetch_add(1, Ordering::SeqCst);

    hyperion_scheduler::stop();
}

/* pub fn verify_outcome(panic_info: Option<&PanicInfo>) {
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
static SUCCESSFUL: AtomicBool = AtomicBool::new(true); */

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
        assert_eq!(0, 1);
    }
}
