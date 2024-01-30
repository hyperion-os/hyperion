#![no_std]

//

extern crate alloc;

use core::fmt::{Arguments, Display};

use arcstr::{literal, ArcStr};
use hyperion_escape::encode::EscapeEncoder;
use spin::Once;

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

    #[must_use]
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

    fn proc_name(&self) -> Option<ArcStr>;

    fn print(&self, level: LogLevel, args: Arguments);
}

//

pub fn set_logger(new_logger: &'static dyn Logger) {
    let mut set = false;
    LOGGER.call_once(|| {
        set = true;
        new_logger
    });

    if !set {
        error!("set_logger: logger was already set");
    }
}

#[doc(hidden)]
pub fn _print_log_custom(level: LogLevel, pre: impl Display, module: &str, args: Arguments) {
    let task = logger()
        .proc_name()
        .unwrap_or(literal!("pre-scheduler"))
        .true_lightgrey()
        .with_reset(false);

    let module = module
        .trim_start_matches("hyperion_")
        .true_grey()
        .with_reset(false);

    logger().print(
        level,
        format_args!(
            "{}{pre}{task} {} {}: {args}",
            '['.true_grey().with_reset(false),
            module,
            ']'.reset_after(),
        ),
    );
}

#[doc(hidden)]
pub fn _print(level: LogLevel, args: Arguments) {
    logger().print(level, args);
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
    _print_log_custom(level, pre, module, args);
}

#[doc(hidden)]
pub fn _is_enabled(level: LogLevel) -> bool {
    logger().is_enabled(level)
}

//

struct NopLogger;

impl Logger for NopLogger {
    fn is_enabled(&self, _: LogLevel) -> bool {
        false
    }

    fn proc_name(&self) -> Option<ArcStr> {
        None
    }

    fn print(&self, _: LogLevel, _: Arguments) {}
}

//

fn logger() -> &'static dyn Logger {
    LOGGER.get().copied().unwrap_or(&NopLogger)
}

//

static LOGGER: Once<&'static dyn Logger> = Once::new();
