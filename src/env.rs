use crate::log::{self, LogLevel};
use spin::Once;

//

pub fn args() -> Arguments {
    Arguments::get()
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arguments {
    pub log_level: LogLevel,
    // log_color: bool,
    pub had_unrecognized: bool,

    pub cmdline: &'static str,
}

//

impl Arguments {
    pub fn parse(s: &'static str) {
        ARGUMENTS.call_once(|| {
            let mut iter = s.split(|c: char| c.is_whitespace() || c == '=');
            let mut result = Arguments::default();
            result.cmdline = s;

            while let Some(item) = iter.next() {
                match item {
                    "log" => {
                        if let Some(level) = iter.next() {
                            if let Some(l) = LogLevel::parse(level) {
                                result.log_level = l
                            } else {
                                result.had_unrecognized = true
                            }
                        }
                    }
                    _ => result.had_unrecognized = true,
                }
            }

            result.assign();

            result
        });
    }

    pub fn get() -> Self {
        ARGUMENTS.get().copied().unwrap_or(Self::default())
    }

    pub fn assign(&self) {
        log::set_log_level(self.log_level);
        // log::set_log_color(self.log_color);
    }
}

impl Default for Arguments {
    fn default() -> Self {
        Self {
            log_level: LogLevel::default(),
            had_unrecognized: false,
            cmdline: "",
        }
    }
}

static ARGUMENTS: Once<Arguments> = Once::new();
