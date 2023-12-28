#![no_std]
#![allow(internal_features)]
#![feature(
    new_uninit,
    const_mut_refs,
    str_split_remainder,
    lang_items,
    never_type
)]

//

use core::fmt::{self, Write};

use hyperion_syscall::exit;

use self::fs::{STDERR, STDOUT};

//

extern crate alloc as core_alloc;

pub mod sys {
    pub use hyperion_syscall::*;
}

pub mod alloc;
pub mod env;
pub mod fs;
pub mod io;
pub mod net;
pub mod process;
pub mod sync;
pub mod thread;

mod rt;

//

#[macro_export]
macro_rules! print {
    ($($v:tt)*) => {
        $crate::_print(format_args!($($v)*))
    };
}

#[macro_export]
macro_rules! eprint {
    ($($v:tt)*) => {
        $crate::_eprint(format_args!($($v)*))
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n");
    };

    ($($v:tt)+) => {
        $crate::print!("{}\n", format_args!($($v)*))
    };
}

#[macro_export]
macro_rules! eprintln {
    () => {
        $crate::eprint!("\n");
    };

    ($($v:tt)*) => {
        $crate::eprint!("{}\n", format_args!($($v)*))
    };
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    _ = STDOUT.lock().write_fmt(args);
}

#[doc(hidden)]
pub fn _eprint(args: fmt::Arguments) {
    _ = STDERR.lock().write_fmt(args);
}

//

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    eprintln!("{info}");
    exit(-1);
}

// to fix `cargo clippy` without a target
#[cfg(feature = "cargo-clippy")]
#[lang = "eh_personality"]
fn eh_personality() {}
