use crate::{print, println};
use core::{any::type_name, panic::PanicInfo};
use x86_64::instructions::port::Port;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub trait TestCase {
    fn run(&self);
}

//

impl<F: Fn()> TestCase for F {
    fn run(&self) {
        let name = type_name::<Self>();
        print!(" - {name:.<40}");
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

pub fn test_runner(tests: &[&dyn TestCase]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        // unsafe {
        //     core::intrinsics::r#try(
        //         move |_| test(),
        //         0 as _,
        //         |_, _| {
        //             println!("[failed]\n");
        //         },
        //     );
        // }

        // TODO: core::panic::catch_unwind // https://github.com/rust-lang/rfcs/issues/2810

        test.run();
    }

    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) {
    println!("[failed]\n{info}\n");
    exit_qemu(QemuExitCode::Failed);
}

#[cfg(test)]
mod tests {
    #[allow(clippy::eq_op)]
    #[test_case]
    fn trivial() {
        assert_eq!(0, 0);
    }

    // TODO: should_panic / should_fail
    #[test_case]
    fn random_tests() {
        // error handling test
        // stack_overflow(79999999);
        // unsafe {
        //     *(0xFFFFFFFFDEADC0DE as *mut u8) = 42;
        // }

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
