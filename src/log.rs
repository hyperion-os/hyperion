use core::fmt::Arguments;
use spin::Lazy;

//

#[macro_export]
macro_rules! print {
    ($($t:tt)*) => { $crate::log::_print(format_args!($($t)*)) };
}

#[macro_export]
macro_rules! println {
    ()          => { $crate::log::_print(format_args!("\n")); };
    ($($t:tt)*) => { $crate::log::_print(format_args_nl!($($t)*)); };
}

//

static LOGGER: Lazy<Logger> = Lazy::new(Logger::init);

struct Logger {
    term: bool,
    qemu: bool,
}

impl Logger {
    fn init() -> Self {
        Logger {
            term: true,
            qemu: true,
        }
    }

    fn print(&self, args: Arguments) {
        if self.term {
            crate::arch::boot::_print(args);
        }
        if self.qemu {
            crate::qemu::_print(args);
        }
    }
}

#[doc(hidden)]
pub fn _print(args: Arguments) {
    LOGGER.print(args)
}
