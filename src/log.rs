use crate::term::escape::encode::EscapeEncoder;
use core::{
    fmt::Arguments,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

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

#[macro_export]
macro_rules! log {
    ($level:expr, $($t:tt)*) => {
        if $crate::log::test_log_level($level) {
            $crate::log::_print_log($level, module_path!(), format_args_nl!($($t)*));
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($t:tt)*) => { $crate::log!($crate::log::LogLevel::Error, $($t)*) };
}

#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => { $crate::log!($crate::log::LogLevel::Warn, $($t)*) };
}

#[macro_export]
macro_rules! info {
    ($($t:tt)*) => { $crate::log!($crate::log::LogLevel::Info, $($t)*) };
}

#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => { $crate::log!($crate::log::LogLevel::Debug, $($t)*) };
}

#[macro_export]
macro_rules! trace {
    ($($t:tt)*) => { $crate::log!($crate::log::LogLevel::Trace, $($t)*) };
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

pub fn set_log_level(level: LogLevel) {
    LOGGER.level.store(level as u8, Ordering::SeqCst);
}

// pub fn set_log_color(color: bool) {
//     LOGGER.color.store(color, Ordering::SeqCst);
// }

pub fn test_log_level(level: LogLevel) -> bool {
    LOGGER.level.load(Ordering::SeqCst) >= level as u8
}

#[doc(hidden)]
pub fn _print_log(level: LogLevel, module: &str, args: Arguments) {
    // if !LOGGER.color.load(Ordering::SeqCst) {
    //     print!("[{level:?}]: ")
    // } else {
    let level = match level {
        LogLevel::None => " NONE  ",
        LogLevel::Error => "\x1b[38;2;255;85;85m ERROR ",
        LogLevel::Warn => "\x1b[38;2;255;255;85m WARN  ",
        LogLevel::Info => "\x1b[38;2;85;255;85m INFO  ",
        LogLevel::Debug => "\x1b[38;2;85;255;255m DEBUG ",
        LogLevel::Trace => "\x1b[38;2;255;85;255m TRACE ",
    };

    print!(
        "{}{level} {} {}: {args}",
        '['.true_grey(),
        module.true_grey(),
        ']'.true_grey(),
    )
    // }
}

#[doc(hidden)]
pub fn _print(args: Arguments) {
    LOGGER.print(args)
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    None,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

//

impl LogLevel {
    pub const DEFAULT: Self = Self::Info;
    pub const ALL: [LogLevel; 5] = [
        Self::Error,
        Self::Warn,
        Self::Info,
        Self::Debug,
        Self::Trace,
    ];

    pub fn parse(s: &str) -> Option<Self> {
        // TODO: match any case
        Some(match s {
            "none" => Self::None,
            "error" => Self::Error,
            "warn" => Self::Warn,
            "info" => Self::Info,
            "debug" => Self::Debug,
            "trace" => Self::Trace,
            _ => return None,
        })
    }
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::DEFAULT
    }
}

//

static LOGGER: Logger = Logger::init();

struct Logger {
    // Log to a bootloader given terminal
    // term: AtomicBool,

    // Log to a framebuffer
    fbo: AtomicBool,

    // Log to a QEMU serial
    qemu: AtomicBool,

    // [`LogLevel`] in u8 form
    level: AtomicU8,
    // print logs with colors
    // color: AtomicBool,
}

impl Logger {
    const fn init() -> Self {
        Logger {
            // term: false.into(),
            fbo: AtomicBool::new(true),
            qemu: AtomicBool::new(true),

            level: AtomicU8::new(LogLevel::DEFAULT as u8),
            // color: AtomicBool::new(true),
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

//

#[cfg(test)]
mod tests {
    use super::{set_log_level, LogLevel};

    #[test_case]
    fn log_levels() {
        set_log_level(LogLevel::Trace);

        for level in LogLevel::ALL {
            log!(level, "LOG TEST")
        }
    }

    #[test_case]
    fn log_chars() {
        for c in 0..=255u8 {
            print!("{}", c as char);
        }
    }
}
