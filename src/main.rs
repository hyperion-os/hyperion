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

use crate::{
    term::escape::encode::EscapeEncoder,
    video::framebuffer::{get_fbo, Color},
};

//

#[path = "arch/x86_64/mod.rs"]
pub mod arch;
pub mod log;
pub mod panic;
pub mod qemu;
pub mod term;
#[cfg(test)]
pub mod testfw;
pub mod video;

//

/// Name of the kernel
pub static KERNEL: &str = if cfg!(test) {
    "Hyperion-Testing"
} else {
    "Hyperion"
};

/// Name of the detected bootloader
pub static BOOTLOADER: Mutex<&'static str> = Mutex::new(KERNEL);

//

fn kernel_main() -> ! {
    println!("\n\nHello from {}", KERNEL.cyan());
    println!(" - {} was booted with {}", KERNEL.cyan(), BOOTLOADER.lock());

    // error handling test
    // stack_overflow(79999999);
    // unsafe {
    //     *(0xFFFFFFFFDEADC0DE as *mut u8) = 42;
    // }

    if let Some(mut fbo) = get_fbo() {
        fbo.fill(240, 340, 40, 40, Color::RED);
        fbo.fill(250, 350, 60, 40, Color::GREEN);
        fbo.fill(205, 315, 80, 20, Color::BLUE);
    }

    #[cfg(test)]
    test_main();

    arch::done();
}

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
