#![no_std]
#![allow(internal_features)]
#![feature(
    new_zeroed_alloc,
    const_mut_refs,
    str_split_remainder,
    lang_items,
    never_type,
    naked_functions
)]

//

use core::fmt;

use self::io::{stderr, stdout, Write};

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
    _ = stdout().lock().write_fmt(args);
}

#[doc(hidden)]
pub fn _eprint(args: fmt::Arguments) {
    _ = stderr().lock().write_fmt(args);
}

//

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    eprintln!("{info}");
    hyperion_syscall::exit(-1);
}

// to fix `cargo clippy` without a target
#[cfg(clippy)]
#[lang = "eh_personality"]
fn eh_personality() {}
