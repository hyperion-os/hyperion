#![doc = include_str!("../README.md")]
//
#![no_std]
#![no_main]
//
#![feature(format_args_nl)]
#![feature(abi_x86_interrupt)]
#![feature(allocator_api)]
#![feature(pointer_is_aligned)]
#![feature(int_roundings)]
#![feature(array_chunks)]
#![feature(cfg_target_has_atomic)]
#![feature(slice_as_chunks)]
#![feature(core_intrinsics)]
//
#![feature(custom_test_frameworks)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]

//

use alloc::string::String;
use chrono::{TimeZone, Utc};
use futures_util::StreamExt;

use crate::{driver::rtc, task::keyboard::KeyboardEvents, util::fmt::NumberPostfix};

use self::vfs::IoResult;

extern crate alloc;

//

#[path = "arch/x86_64/mod.rs"]
pub mod arch;
pub mod boot;
pub mod driver;
pub mod log;
pub mod mem;
pub mod panic;
pub mod smp;
pub mod task;
pub mod term;
#[cfg(test)]
pub mod testfw;
pub mod util;
pub mod vfs;

//

pub static KERNEL_NAME: &str = if cfg!(test) {
    "Hyperion-Testing"
} else {
    "Hyperion"
};

pub static KERNEL_VERSION: &str = env!("CARGO_PKG_VERSION");

//

// the actual entry exists in [´crate::boot::boot´]
fn kernel_main() -> ! {
    debug!("Entering kernel_main");

    arch::early_boot_cpu();

    debug!("Cmdline: {:?}", boot::args::get());

    debug!(
        "Kernel addr: {:?} ({}B), {:?} ({}B), ",
        boot::virt_addr(),
        boot::virt_addr().as_u64().postfix_binary(),
        boot::phys_addr(),
        boot::phys_addr().as_u64().postfix_binary(),
    );
    debug!("HHDM Offset: {:#0X?}", boot::hhdm_offset());

    // ofc. every kernel has to have this cringy ascii name splash
    info!("\n{}\n", include_str!("./splash"));

    if let Some(bl) = boot::BOOT_NAME.get() {
        debug!("{KERNEL_NAME} {KERNEL_VERSION} was booted with {bl}");
    }

    #[cfg(test)]
    test_main();

    if let Some(time) = rtc::RTC.now() {
        debug!("RTC time: {time:?}");
    }

    rtc::RTC.enable_ints();
    rtc::Rtc::install_device();
    debug!(
        "ints enabled?: {}",
        x86_64::instructions::interrupts::are_enabled()
    );

    // main task(s)
    task::spawn(shell());

    // jumps to [smp_main] right bellow + wakes up other threads to jump there
    smp::init()
}

fn smp_main(cpu: smp::Cpu) -> ! {
    debug!("{cpu} entering smp_main");

    arch::early_per_cpu(&cpu);

    task::run_tasks();
}

async fn shell() {
    let mut ev = KeyboardEvents::new();
    let mut cmdbuf = String::new();
    print!("\n[shell] > ");
    while let Some(ev) = ev.next().await {
        if ev == '\n' {
            println!();
            if let Err(err) = run_line(&cmdbuf).await {
                println!("err: {err:?}");
            };
            cmdbuf.clear();
            print!("\n[shell] > ");
        } else if ev == '\u{8}' {
            cmdbuf.pop();
            print!("\n[shell] > {cmdbuf}");
        } else {
            print!("{ev}");
            cmdbuf.push(ev);
        }
    }
}

async fn run_line(line: &str) -> IoResult<()> {
    let (cmd, args) = line
        .split_once(' ')
        .map(|(cmd, args)| (cmd, Some(args)))
        .unwrap_or((line, None));

    match cmd {
        "ls" => {
            let dir = vfs::get_dir(args.unwrap_or("/"), false)?;
            let mut dir = dir.lock();
            for entry in dir.nodes()? {
                println!("{entry}");
            }
        }
        "cat" => {
            let file = vfs::get_file(args.unwrap_or("/"), false, false)?;
            let mut file = file.lock();

            let mut at = 0usize;
            let mut buf = [0u8; 16];
            loop {
                let read = file.read(at, &mut buf)?;
                if read == 0 {
                    break;
                }
                at += read;

                for byte in buf {
                    print!("{byte:#02} ");
                }
                println!();
            }
        }
        "date" => {
            let file = vfs::get_file("/dev/rtc", false, false)?;
            let mut file = file.lock();

            let mut timestamp = [0u8; 8];
            file.read_exact(0, &mut timestamp)?;

            let date = Utc.timestamp_nanos(i64::from_le_bytes(timestamp));

            println!("{date:?}");
        }
        other => {
            println!("unknown command {other}");
        }
    }

    Ok(())
}
