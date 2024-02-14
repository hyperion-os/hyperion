use core::fmt;

//

#[derive(Debug, Clone, Copy)]
pub enum Message {
    NewWindow { window_id: usize },
    Event(Event),
}

impl Message {
    pub fn parse(line: &str) -> Option<Self> {
        let (ty, data) = line.split_once(' ').unwrap_or((line, ""));

        match ty {
            "new_window" => {
                let Ok(window_id) = data.parse::<usize>() else {
                    eprintln!("invalid new_window data");
                    return None;
                };

                Some(Self::NewWindow { window_id })
            }
            "event" => Event::parse(data).map(Self::Event),
            _ => {
                eprintln!("unknown result type `{ty}`");
                None
            }
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Message::NewWindow { window_id } => {
                write!(f, "new_window {window_id}")
            }
            Message::Event(event) => {
                write!(f, "event {event}")
            }
        }
    }
}

//

#[derive(Debug, Clone, Copy)]
pub enum Event {
    Keyboard { code: u8, state: u8 },
    Text { ch: char },
}

impl Event {
    pub fn parse(data: &str) -> Option<Self> {
        let (ty, data) = data.split_once(' ').unwrap_or((data, ""));

        match ty {
            "keyboard" => {
                let Some((code, state)) = data.split_once(' ') else {
                    eprintln!("invalid event keyboard data");
                    return None;
                };
                let (Ok(code), Ok(state)) = (code.parse::<u8>(), state.parse::<u8>()) else {
                    eprintln!("invalid event keyboard data");
                    return None;
                };

                Some(Event::Keyboard { code, state })
            }
            "text" => {
                let Some(ch) = data.parse::<u32>().ok().and_then(|ch| char::from_u32(ch)) else {
                    eprintln!("invalid event text data");
                    return None;
                };

                Some(Event::Text { ch })
            }
            _ => {
                eprintln!("unknown event type `{ty}`");
                None
            }
        }
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Event::Keyboard { code, state } => {
                write!(f, "keyboard {code} {state}")
            }
            Event::Text { ch } => {
                write!(f, "text {}", ch as u32)
            }
        }
    }
}

//

#[derive(Debug, Clone, Copy)]
pub enum Request {
    NewWindow,
}

impl Request {
    pub fn parse(line: &str) -> Option<Self> {
        let (ty, _) = line.split_once(' ').unwrap_or((line, ""));

        match ty {
            "new_window" => Some(Self::NewWindow),
            _ => {
                eprintln!("unknown request type `{ty}`");
                None
            }
        }
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Request::NewWindow => write!(f, "new_window"),
        }
    }
}
