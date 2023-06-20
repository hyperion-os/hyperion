//

pub use pc_keyboard::KeyCode;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardEvent {
    // pub scancode: u8,
    pub state: ElementState,
    pub keycode: KeyCode,
    pub unicode: Option<char>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementState {
    PressHold,
    PressRelease,
    Release,
}
