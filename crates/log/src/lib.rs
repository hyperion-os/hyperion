#![no_std]
#![feature(format_args_nl)]

//

use core::fmt::Arguments;

use spin::Once;

//

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        $crate::_print(format_args!("{}\n", format_args!($($arg)*)))
    };
}

#[doc(hidden)]
pub fn _print(args: Arguments) {
    if let Some(logger) = LOGGER.get() {
        logger.print(args);
    }
}

pub fn init_logger(logger: &'static dyn Logger) {
    LOGGER.call_once(|| logger);
}

//

pub trait Logger: Sync + Send {
    fn print(&self, args: Arguments);
}

//

static LOGGER: Once<&'static dyn Logger> = Once::new();
