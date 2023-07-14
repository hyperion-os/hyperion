#![doc = include_str!("../../README.md")]
//
#![no_std]
#![no_main]
//
#![feature(
    const_option,
    allocator_api,
    pointer_is_aligned,
    int_roundings,
    array_chunks,
    core_intrinsics,
    custom_test_frameworks,
    panic_can_unwind,
    lang_items
)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]

//

use hyperion_boot_interface::Cpu;
use hyperion_kernel_info::{NAME, VERSION};
use hyperion_log::debug;

extern crate alloc;

//

pub mod panic;
pub mod syscall;
#[cfg(test)]
pub mod testfw;

//

#[no_mangle]
fn kernel_main() -> ! {
    // enable logging and and outputs based on the kernel args,
    // any logging before won't be shown
    hyperion_log_multi::init_logger();

    debug!("Entering kernel_main");
    debug!("{NAME} {VERSION} was booted with {}", hyperion_boot::NAME);

    hyperion_arch::early_boot_cpu();

    hyperion_arch::syscall::set_handler(syscall::syscall);

    /* // set syscall int handler
    hyperion_interrupts::set_interrupt_handler(0xAA, || {
        hyperion_log::println!("got syscall");
    }); */

    hyperion_random::provide_entropy(&hyperion_arch::rng_seed().to_ne_bytes());

    hyperion_drivers::lazy_install_early();

    #[cfg(test)]
    test_main();

    // main task(s)
    hyperion_scheduler::spawn(hyperion_kshell::kshell());

    // jumps to [smp_main] right bellow + wakes up other threads to jump there
    hyperion_boot::smp_init(smp_main);
}

fn smp_main(cpu: Cpu) -> ! {
    debug!("{cpu} entering smp_main");

    hyperion_arch::early_per_cpu(&cpu);

    if cpu.is_boot() {
        hyperion_drivers::lazy_install_late();
    }

    hyperion_scheduler::run_tasks();
}

// for clippy:
#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
