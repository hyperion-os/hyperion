pub use pc_keyboard::KeyCode;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardEvent {
    // pub scancode: u8,
    pub state: ElementState,
    pub keycode: KeyCode,
    pub unicode: Option<char>,
}

impl KeyboardEvent {
    pub(crate) const fn empty() -> Self {
        Self {
            state: ElementState::Pressed,
            keycode: KeyCode::Escape,
            unicode: None,
        }
    }
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementState {
    Pressed,
    Released,
}
