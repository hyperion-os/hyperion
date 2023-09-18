#![no_std]
#![no_main]

use core::fmt::{self, Write};

use hyperion_syscall::{exit, log, timestamp, yield_now};

//

struct SyscallLog;

//

impl Write for SyscallLog {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if hyperion_syscall::log(s) == 0 {
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}

//

#[no_mangle]
pub extern "C" fn _start() -> ! {
    writeln!(
        &mut SyscallLog,
        "sample app main\nints enabled?: {}",
        x86_64::instructions::interrupts::are_enabled()
    )
    .unwrap();

    // page fault test:
    /* writeln!(&mut SyscallLog, "sample-elf page fault test").unwrap();
    let null_ptr = core::hint::black_box(0x0) as *const u8;
    core::hint::black_box(unsafe { *null_ptr }); */

    let mut next = 0;
    for i in 0.. {
        while timestamp().unwrap() < next {
            yield_now();
        }
        next += 1_000_000_000;

        writeln!(&mut SyscallLog, "testing `{i}`").unwrap();
    }

    exit(0);
}

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    log("panic");
    exit(-1);
}
