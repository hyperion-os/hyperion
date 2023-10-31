//! Tests should only be ran on a single thread at the moment

use alloc::{format, string::String};
use core::{
    any::type_name,
    panic::PanicInfo,
    sync::atomic::{AtomicUsize, Ordering},
};

use crossbeam::queue::SegQueue;
use hyperion_log::{error, print, println, LogLevel};
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

    for test in tests {
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

    hyperion_scheduler::schedule(move || {
        hyperion_scheduler::rename("testfw waiter".into());
        loop {
            let completed = TESTS_COMPLETE.load(Ordering::SeqCst);
            // println!("completed: {completed}");
            if completed != tests.len() {
                hyperion_scheduler::yield_now();
                continue;
            }

            let mut fails = false;
            while let Some(err) = TESTS_FAILS.pop() {
                error!("ERROR: {err}");
                fails = true;
            }

            if fails {
                exit_qemu(QemuExitCode::Failed)
            } else {
                exit_qemu(QemuExitCode::Success)
            }
        }
    });
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    let name = hyperion_scheduler::task().name.read().clone();
    let should_panic = name.ends_with("should_panic");

    if !should_panic {
        TESTS_FAILS.push(format!("`{name}` paniced unexpectedly: {info}"));
        println!("[err]")
    } else {
        println!("[ok]")
    }
    TESTS_COMPLETE.fetch_add(1, Ordering::SeqCst);

    hyperion_scheduler::stop();
}

//

static TESTS_COMPLETE: AtomicUsize = AtomicUsize::new(0);
static TESTS_FAILS: SegQueue<String> = SegQueue::new();

//

#[cfg(test)]
mod tests {

    #[allow(clippy::eq_op)]
    #[test_case]
    fn trivial() {
        assert_eq!(0, 0);
    }

    // mark this test to be a panic=success
    #[test_case]
    fn should_panic() {
        assert_eq!(0, 1);
    }

    #[test_case]
    fn should_panic_test() {
        // assert_eq!(0, 1); // should panic AND fail
    }
}
