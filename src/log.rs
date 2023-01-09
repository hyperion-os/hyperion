use core::{
    fmt::Arguments,
    sync::atomic::{AtomicBool, Ordering},
};
use spin::Lazy;

//

#[macro_export]
macro_rules! print {
    ($($t:tt)*) => { $crate::log::_print(format_args!($($t)*)) };
}

#[macro_export]
macro_rules! println {
    ()          => { $crate::log::_print(format_args!("\n")) };
    ($($t:tt)*) => { $crate::log::_print(format_args_nl!($($t)*)) };
}

//

pub fn enable_term() {
    LOGGER.term.store(true, Ordering::SeqCst);
}

pub fn disable_term() {
    LOGGER.term.store(false, Ordering::SeqCst);
}

pub fn enable_qemu() {
    LOGGER.qemu.store(true, Ordering::SeqCst);
}

pub fn disable_qemu() {
    LOGGER.qemu.store(false, Ordering::SeqCst);
}

//

static LOGGER: Lazy<Logger> = Lazy::new(Logger::init);

struct Logger {
    term: AtomicBool,
    qemu: AtomicBool,
}

impl Logger {
    fn init() -> Self {
        Logger {
            term: true.into(),
            qemu: true.into(),
        }
    }

    fn print(&self, args: Arguments) {
        if self.qemu.load(Ordering::SeqCst) {
            crate::qemu::_print(args);
        }
        if self.term.load(Ordering::SeqCst) {
            crate::arch::boot::_print(args);
        }
    }
}

#[doc(hidden)]
pub fn _print(args: Arguments) {
    LOGGER.print(args)
}
