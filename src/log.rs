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

// pub fn enable_term() {
//     LOGGER.term.store(true, Ordering::SeqCst);
// }
//
// pub fn disable_term() {
//     LOGGER.term.store(false, Ordering::SeqCst);
// }

pub fn enable_fbo() {
    LOGGER.fbo.store(true, Ordering::SeqCst);
}

pub fn disable_fbo() {
    LOGGER.fbo.store(false, Ordering::SeqCst);
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
    // Log to a bootloader given terminal
    // term: AtomicBool,

    // Log to a framebuffer
    fbo: AtomicBool,

    // Log to a QEMU serial
    qemu: AtomicBool,
}

impl Logger {
    fn init() -> Self {
        Logger {
            // term: false.into(),
            fbo: true.into(),
            qemu: true.into(),
        }
    }

    fn print(&self, args: Arguments) {
        // if self.term.load(Ordering::SeqCst) {
        //     crate::arch::boot::_print(args);
        // }
        if self.qemu.load(Ordering::SeqCst) {
            crate::qemu::_print(args);
        }
        if self.fbo.load(Ordering::SeqCst) {
            crate::video::logger::_print(args);
        }
    }
}

#[doc(hidden)]
pub fn _print(args: Arguments) {
    LOGGER.print(args)
}
