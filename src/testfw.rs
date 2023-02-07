use crate::{log, print, println};
use core::{
    any::type_name,
    panic::PanicInfo,
    sync::atomic::{AtomicUsize, Ordering},
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
        println!("[ok]");
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
        next_test.run();
    }
}

pub fn test_panic_handler(info: &PanicInfo) {
    println!("[failed]\n{info}\n");
    // a hack to keep running tests even tho a panic happened
    run_tests();
    exit_qemu(QemuExitCode::Failed);
}

static TESTS: Once<&'static [&'static dyn TestCase]> = Once::new();
static IDX: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
mod tests {
    /* use crate::{debug, println}; */

    #[allow(clippy::eq_op)]
    #[test_case]
    fn trivial() {
        assert_eq!(0, 1);
    }

    // TODO: should_panic / should_fail
    #[test_case]
    fn random_tests() {
        // error handling test

        /* stack_overflow(79999999); */

        /* unsafe {
            *(0xFFFFFFFFDEADC0DE as *mut u8) = 42;
        } */

        /* unsafe {
            let x = *(0xffffffffc18a8137 as *mut u8);
            println!("Read worked: {x}");
            *(0xffffffffc18a8137 as *mut u8) = 42;
            println!("Write worked");
        } */

        #[allow(unused)]
        fn stack_overflow(n: usize) {
            if n == 0 {
                return;
            } else {
                stack_overflow(n - 1);
            }
            unsafe {
                core::ptr::read_volatile(&0 as *const i32);
            }
        }
    }
}
