#![no_std]

//

use core::fmt::{Arguments, Display};

use hyperion_escape::encode::EscapeEncoder;
use spin::RwLock;

//

#[macro_export]
macro_rules! print {
    ($($t:tt)*) => {
        $crate::_print($crate::LogLevel::Info, format_args!($($t)*))
    };
}

#[macro_export]
macro_rules! println {
    ()          => {
        $crate::print!("\n")
    };
    ($($t:tt)*) => {
        $crate::print!("{}\n", format_args!($($t)*))
    };
}

#[macro_export]
macro_rules! log {
    ($level:expr, $($t:tt)*) => {
        if $crate::_is_enabled($level) {
            $crate::_print_log($level, module_path!(), format_args!("{}\n", format_args!($($t)*)))
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($t:tt)*) => {
        $crate::log!($crate::LogLevel::Error, $($t)*)
    };
}

#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => {
        $crate::log!($crate::LogLevel::Warn, $($t)*)
    };
}

#[macro_export]
macro_rules! info {
    ($($t:tt)*) => {
        $crate::log!($crate::LogLevel::Info, $($t)*)
    };
}

#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => {
        $crate::log!($crate::LogLevel::Debug, $($t)*)
    };
}

#[macro_export]
macro_rules! trace {
    ($($t:tt)*) => {
        $crate::log!($crate::LogLevel::Trace, $($t)*)
    };
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

pub trait Logger: Send + Sync {
    fn is_enabled(&self, level: LogLevel) -> bool;

    fn print(&self, level: LogLevel, args: Arguments);
}

//

pub fn set_logger(new_logger: &'static dyn Logger) {
    *LOGGER.write() = new_logger;
}

pub fn _print_log_custom(level: LogLevel, pre: impl Display, module: &str, args: Arguments) {
    _print(
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
pub fn _print(level: LogLevel, args: Arguments) {
    LOGGER.read().print(level, args)
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
    _print_log_custom(level, pre, module, args)
}

pub fn _is_enabled(level: LogLevel) -> bool {
    LOGGER.read().is_enabled(level)
}

//

static LOGGER: RwLock<&'static dyn Logger> = RwLock::new(&NopLogger);

//

struct NopLogger;

impl Logger for NopLogger {
    fn is_enabled(&self, _: LogLevel) -> bool {
        false
    }

    fn print(&self, _: LogLevel, _: Arguments) {}
}
