#![doc = include_str!("../../../README.md")]
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
    type_name_of_val
)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(clippy::needless_return)]

//

use core::ops::Range;

use hyperion_arch as arch;
use hyperion_boot as boot;
use hyperion_boot_interface::Cpu;
use hyperion_drivers as drivers;
use hyperion_futures as futures;
use hyperion_kernel_info::{NAME, VERSION};
use hyperion_kshell as kshell;
use hyperion_log::*;
use hyperion_log_multi as log_multi;
use hyperion_mem::from_higher_half;
use hyperion_random as random;
use hyperion_scheduler as scheduler;
use spin::Once;
use x86_64::VirtAddr;

extern crate alloc;

//

pub mod panic;
pub mod syscall;
#[cfg(test)]
pub mod testfw;

//

static BSP_BOOT_STACK: Once<Range<VirtAddr>> = Once::new();

//

#[no_mangle]
extern "C" fn _start() -> ! {
    let boot_stack = arch::stack_pages();
    BSP_BOOT_STACK.call_once(move || boot_stack);

    // enable logging and and outputs based on the kernel args,
    // any logging before won't be shown
    log_multi::init_logger();

    debug!("Entering kernel_main");
    debug!("{NAME} {VERSION} was booted with {}", boot::NAME);

    //
    arch::syscall::set_handler(syscall::syscall);
    arch::init_bsp_cpu();

    random::provide_entropy(&arch::rng_seed().to_ne_bytes());

    drivers::lazy_install_early();

    #[cfg(test)]
    test_main();

    // main task(s)
    futures::executor::spawn(kshell::kshell());

    // jumps to [smp_main] right bellow + wakes up other threads to jump there
    boot::smp_init(smp_main);
}

fn smp_main(cpu: Cpu) -> ! {
    let mut boot_stack = arch::stack_pages();

    trace!("{cpu} entering smp_main");

    arch::init_smp_cpu(&cpu);

    if cpu.is_boot() {
        boot_stack = BSP_BOOT_STACK
            .get()
            .expect("_start to run before smp_main")
            .clone();

        drivers::lazy_install_late();
        debug!("boot cpu drivers installed");
    }

    trace!("boot stack: {boot_stack:?}");

    scheduler::schedule(move || {
        scheduler::rename("<kernel futures executor>".into());

        let first = from_higher_half(boot_stack.start);
        let count = ((boot_stack.end - boot_stack.start) / 0x1000) as usize;

        let frames = unsafe { hyperion_mem::pmm::PageFrame::new(first, count) };
        // debug!("deallocating bootloader provided stack");
        hyperion_mem::pmm::PFA.free(frames);

        futures::executor::run_tasks();
    });
    trace!("resetting {cpu} scheduler");
    scheduler::init();
}

//

#[cfg(test)]
mod tests {
    use alloc::{
        string::{String, ToString},
        vec::Vec,
    };

    use hyperion_arch as arch;
    use hyperion_log::*;
    use hyperion_scheduler as scheduler;

    #[test_case]
    fn scheduler_simple_ipc_test() {
        let self_pid = scheduler::process().pid;

        let task_3_pid = scheduler::schedule(move || {
            scheduler::rename("<Find_Missing>".into());

            // let pid = scheduler::process().pid;
            // info!("I am pid:{pid}");

            loop {
                let data = scheduler::recv();
                let string = core::str::from_utf8(&data).expect("data to be UTF-8");

                let mut found = [false; 26];
                for c in string.chars() {
                    found[((c as u8).to_ascii_lowercase() - b'a') as usize] = true;
                }

                let mut buf = String::new();
                for missing in found
                    .iter()
                    .enumerate()
                    .filter(|(_, found)| !*found)
                    .map(|(i, _)| i)
                {
                    buf.push((missing as u8 + b'a') as char);
                }
                // println!("<Find_Missing>: '{buf}'");

                scheduler::send(self_pid, Vec::from(buf).into()).expect("send err");
            }
        });

        let task_2_pid = scheduler::schedule(move || {
            scheduler::rename("<Clean_Input>".into());

            // let pid = scheduler::process().pid;
            // info!("I am pid:{pid}");

            loop {
                let messy_data = scheduler::recv();
                let messy_string = core::str::from_utf8(&messy_data).expect("data to be UTF-8");

                let clean_string = messy_string.replace(|c| !char::is_alphabetic(c), "");
                // println!("<Clean_Input>: '{clean_string}'");

                scheduler::send(task_3_pid, Vec::from(clean_string).into()).expect("send err");
            }
        });

        scheduler::schedule(move || {
            scheduler::rename("<Get_Input>".into());

            // let pid = scheduler::process().pid;
            // info!("I am pid:{pid}");

            loop {
                let messy_string = "abc3de5fgh@lmno&pqr%stuv(w)xyz".to_string();
                // println!("<Get_Input>: '{messy_string}'");
                scheduler::send(task_2_pid, Vec::from(messy_string).into()).expect("send err");

                // wait 2500ms
                scheduler::sleep(time::Duration::milliseconds(2500));
            }
        });

        let result = scheduler::recv();
        assert_eq!(&result[..], &b"ijk"[..])
    }
}
