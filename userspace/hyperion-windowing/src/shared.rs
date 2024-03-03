use serde::{Deserialize, Serialize};

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionClosed;

//

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Message {
    NewWindow { window_id: usize },
    // ResizeWindow { window_id: usize },
    Event(Event),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Window {
    /// user data for the window, for window identification in multi-window scenarios
    pub id: usize,
    /// window buffer pixel pitch (pixels to the same X at the next Y)
    pub pitch: usize,
    /// window visual pixel width
    pub width: usize,
    /// window visual pixel height
    pub height: usize,
    /// FIXME: should be a file descriptor over socket (sendmsg)
    /// a file id of the window fb file `/run/wm.window.<id>`
    pub shmem_file: usize,
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
    // ResizeWindow {
    //     window_id: usize,
    //     width: usize,
    //     height: usize,
    // },
    CloseConnection,
}
