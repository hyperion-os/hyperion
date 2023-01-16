#![no_std]
#![no_main]
#![feature(format_args_nl)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![feature(type_alias_impl_trait)]
#![feature(result_option_inspect)]
#![feature(allocator_api)]
#![feature(nonnull_slice_from_raw_parts)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]

//

use crate::term::escape::encode::EscapeEncoder;
use spin::Once;

//

extern crate alloc;

//

#[path = "arch/x86_64/mod.rs"]
pub mod arch;
pub mod boot;
pub mod env;
pub mod log;
pub mod mem;
pub mod panic;
pub mod qemu;
pub mod smp;
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
pub static BOOTLOADER: Once<&'static str> = Once::new();

//

fn kernel_main() -> ! {
    debug!("Entering kernel_main");
    debug!("Cmdline: {:?}", env::Arguments::get());

    mem::init();

    // ofc. every kernel has to have this cringy ascii name splash
    info!("\n{}\n", include_str!("./splash"));

    if let Some(bl) = BOOTLOADER.get() {
        let kernel = KERNEL.true_cyan();
        debug!("{kernel} was booted with {bl}");
    }

    #[cfg(test)]
    test_main();

    smp::init();
}
