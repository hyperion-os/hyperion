use crossbeam::atomic::AtomicCell;

use crate::term::escape::encode::{EncodedPart, EscapeEncoder};
use core::fmt::Arguments;

//

#[macro_export]
macro_rules! print {
    ($($t:tt)*) => { $crate::log::_print($crate::log::LogLevel::Info, format_args!($($t)*)) };
}

#[macro_export]
macro_rules! println {
    ()          => { $crate::log::_print($crate::log::LogLevel::Info, format_args!("\n")) };
    ($($t:tt)*) => { $crate::log::_print($crate::log::LogLevel::Info, format_args_nl!($($t)*)) };
}

#[macro_export]
macro_rules! log {
    ($level:expr, $($t:tt)*) => {
        $crate::log::_print_log($level, module_path!(), format_args_nl!($($t)*));
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

pub fn set_fbo(level: LogLevel) {
    LOGGER.fbo.store(level)
}

pub fn set_qemu(level: LogLevel) {
    LOGGER.qemu.store(level)
}

/* // pub fn enable_term() {
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

pub fn get_log_level() -> LogLevel {
    match LOGGER.level.load(Ordering::SeqCst) {
        0 => LogLevel::None,
        1 => LogLevel::Error,
        2 => LogLevel::Warn,
        3 => LogLevel::Info,
        4 => LogLevel::Debug,
        5.. => LogLevel::Trace,
    }
}

// pub fn set_log_color(color: bool) {
//     LOGGER.color.store(color, Ordering::SeqCst);
// }

pub fn test_log_level(level: LogLevel) -> bool {
    LOGGER.level.load(Ordering::SeqCst) >= level as u8
} */

pub fn print_log_splash(
    level: LogLevel,
    pre: EncodedPart<'_, &str>,
    module: &str,
    args: Arguments,
) {
    crate::log::_print(
        level,
        format_args!(
            "{}{pre} {} {}: {args}",
            '['.true_grey(),
            module.true_grey().with_reset(false),
            ']'.reset_after(),
        ),
    )
}

#[doc(hidden)]
pub fn _print_log(level: LogLevel, module: &str, args: Arguments) {
    let pre = match level {
        LogLevel::None => " NONE  ".into(),
        LogLevel::Error => " ERROR ".true_red(),
        LogLevel::Warn => " WARN  ".true_yellow(),
        LogLevel::Info => " INFO  ".true_green(),
        LogLevel::Debug => " DEBUG ".true_cyan(),
        LogLevel::Trace => " TRACE ".true_magenta(),
    }
    .with_reset(false);
    print_log_splash(level, pre, module, args)
}

#[doc(hidden)]
pub fn _print(level: LogLevel, args: Arguments) {
    LOGGER.print(level, args)
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
    // Log to a framebuffer
    fbo: AtomicCell<LogLevel>,

    // Log to a QEMU serial
    qemu: AtomicCell<LogLevel>,
}

impl Logger {
    const fn init() -> Self {
        Logger {
            fbo: AtomicCell::new(LogLevel::DEFAULT),
            qemu: AtomicCell::new(LogLevel::DEFAULT),
        }
    }

    fn print(&self, level: LogLevel, args: Arguments) {
        if self.qemu.load() >= level {
            crate::driver::qemu::_print(args);
        }
        if self.fbo.load() >= level {
            crate::driver::video::logger::_print(args);
        }
    }
}
