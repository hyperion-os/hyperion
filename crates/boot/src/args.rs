use hyperion_log::LogLevel;
use spin::Lazy;

//

pub fn get() -> Arguments {
    Arguments::get()
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Arguments {
    pub serial_log_level: LogLevel,
    pub video_log_level: LogLevel,
    // log_color: bool,
    pub had_unrecognized: bool,

    pub cmdline: &'static str,
}

//

impl Arguments {
    pub fn parse(s: &'static str) -> Self {
        let iter = s.split(|c: char| c.is_whitespace());
        let mut result = Arguments {
            cmdline: s,
            ..<_>::default()
        };

        for item in iter {
            let (item, value) = item
                .split_once('=')
                .map(|(item, value)| (item, Some(value)))
                .unwrap_or((item, None));

            match item {
                "log" => {
                    let Some(values) = value else {
                        result.had_unrecognized = true;
                        continue;
                    };

                    for level_or_kvp in values.split(',') {
                        if let Some((dev, level)) = level_or_kvp.split_once('=') {
                            let dev = match dev {
                                "serial" => &mut result.serial_log_level,
                                "video" => &mut result.video_log_level,
                                _other => {
                                    result.had_unrecognized = true;
                                    continue;
                                }
                            };
                            let Some(level) = LogLevel::parse(level) else {
                            result.had_unrecognized = true;
                            continue;
                        };

                            *dev = level;
                        } else {
                            let Some(level) = LogLevel::parse(level_or_kvp) else {
                            result.had_unrecognized = true;
                            continue;
                        };
                            result.serial_log_level = level;
                            result.video_log_level = level;
                        };
                    }
                }
                _ => result.had_unrecognized = true,
            }
        }

        result
    }

    pub fn get() -> Self {
        static ARGUMENTS: Lazy<Arguments> = Lazy::new(|| {
            crate::cmdline()
                .map(Arguments/*Self doesn't work??*/::parse)
                .unwrap_or_default()
        });
        *ARGUMENTS
    }
}
