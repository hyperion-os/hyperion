#![no_std]
#![no_main]
#![feature(format_args_nl)]

use core::fmt::{self, Write};

use hyperion_syscall::*;

//

pub fn main() {
    println!("sample app main");

    let mut next = timestamp().unwrap() as u64;
    for i in 0.. {
        nanosleep_until(next);
        next += 1_000_000_000;

        println!("seconds since boot: {i}");
    }
}

//

#[macro_export]
macro_rules! println {
    ($($v:tt)*) => {
        _print(format_args_nl!($($v)*));
    };
}

//

fn _print(args: fmt::Arguments) {
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

    _ = SyscallLog.write_fmt(args);
}

//

#[no_mangle]
extern "C" fn _start() -> ! {
    main();
    exit(0);
}

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    println!("{info}");
    exit(-1);
}
