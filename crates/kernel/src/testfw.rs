//! Tests should only be ran on a single thread at the moment

// extern crate test;

use alloc::{format, string::String};
use core::{any::type_name, panic::PanicInfo};

use arcstr::ArcStr;
use crossbeam::queue::SegQueue;
use hyperion_log::{print, println, LogLevel};
use hyperion_scheduler::yield_now;
use x86_64::instructions::port::Port;

//

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
        hyperion_scheduler::spawn(move || {
            let name = test.name();
            // println!("running {name}");
            hyperion_scheduler::rename(name);

            test.run();

            let should_panic = name.ends_with("should_panic");
            if should_panic {
                RESULTS.push((
                    name.into(),
                    Some(format!("`{name}` was expected to panic but it didn't")),
                ));
            } else {
                RESULTS.push((name.into(), None));
            }
        });
    }

    hyperion_scheduler::spawn(move || {
        hyperion_scheduler::rename("testfw waiter");

        let mut completed = 0;

        let mut fails = false;
        while let Some((name, err)) = RESULTS.pop() {
            completed += 1;
            print!(" - {name:.<60}");
            if let Some(err) = err {
                println!("[err]");
                println!("ERROR: {err}");
                fails = true;
            } else {
                println!("[ok]");
            }

            if completed == tests.len() {
                break;
            }

            yield_now();
        }

        if fails {
            exit_qemu(QemuExitCode::Failed)
        } else {
            exit_qemu(QemuExitCode::Success)
        }
    });
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    let name = hyperion_scheduler::task().name.read().clone();
    let should_panic = name.ends_with("should_panic");

    if !should_panic {
        let err = Some(format!("`{name}` panicked unexpectedly: {info}"));
        RESULTS.push((name, err));
    } else {
        RESULTS.push((name, None));
    }

    hyperion_scheduler::stop();
}

//

static RESULTS: SegQueue<(ArcStr, Option<String>)> = SegQueue::new();

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
