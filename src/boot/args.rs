use crate::{
    boot,
    log::{self, LogLevel},
};
use spin::{Lazy, Once};

//

pub fn get() -> Arguments {
    Arguments::get()
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Arguments {
    pub log_level: LogLevel,
    // log_color: bool,
    pub had_unrecognized: bool,

    pub cmdline: &'static str,
}

//

impl Arguments {
    pub fn parse(s: &'static str) -> Self {
        let mut iter = s.split(|c: char| c.is_whitespace() || c == '=');
        let mut result = Arguments {
            cmdline: s,
            ..<_>::default()
        };

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

        result
    }

    pub fn get() -> Self {
        static ARGUMENTS: Lazy<Arguments> = Lazy::new(|| {
            boot::cmdline()
                .map(Arguments/*Self doesn't work??*/::parse)
                .unwrap_or_default()
        });
        *ARGUMENTS
    }

    pub fn apply(&self) {
        log::set_log_level(self.log_level);
        // log::set_log_color(self.log_color);
    }
}
