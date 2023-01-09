#![no_std]
#![no_main]
#![feature(format_args_nl)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![feature(type_alias_impl_trait)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]

//

use spin::Mutex;

use crate::video::framebuffer::{Color, FBO};

//

#[path = "arch/x86_64/mod.rs"]
pub mod arch;
pub mod log;
pub mod panic;
pub mod qemu;
#[cfg(test)]
pub mod testfw;
pub mod video;

//

/// Name of the kernel
pub static KERNEL: &'static str = if cfg!(test) {
    "Hyperion-Testing"
} else {
    "Hyperion"
};

/// Name of the detected bootloader
pub static BOOTLOADER: Mutex<&'static str> = Mutex::new(KERNEL);

//

fn kernel_main() -> ! {
    println!("Hello from {KERNEL}");
    println!(" - {KERNEL} was booted with {}", BOOTLOADER.lock());

    // error handling test
    // stack_overflow(79999999);
    // unsafe {
    //     *(0xFFFFFFFFDEADC0DE as *mut u8) = 42;
    // }

    if let Some(fbo) = FBO.get() {
        let mut fbo = fbo.lock();
        fbo.fill(40, 40, 40, 40, Color::RED);
        fbo.fill(50, 50, 60, 40, Color::GREEN);
        fbo.fill(5, 15, 80, 20, Color::BLUE);
    }

    #[cfg(test)]
    test_main();

    arch::done();
}

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
