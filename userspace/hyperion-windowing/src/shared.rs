use serde::{Deserialize, Serialize};

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionClosed;

//

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Message {
    NewWindow { window_id: usize },
    Event(Event),
}

//

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Event {
    Keyboard { code: u8, state: ElementState },
    Text { ch: char },
    Mouse(Mouse),
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElementState {
    Pressed,
    Released,
}

//

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Mouse {
    Motion { x: f32, y: f32 },
    Button { btn: Button, state: ElementState },
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Button {
    Left,
    Middle,
    Right,
}

//

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Request {
    NewWindow,
}
