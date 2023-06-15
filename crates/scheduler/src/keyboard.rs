use core::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Stream;
use hyperion_keyboard::wait_keyboard_event;

//

pub const fn keyboard_events() -> KeyboardEvents {
    KeyboardEvents::new()
}

//

#[derive(Debug, Clone, Copy, Default)]
#[must_use]
pub struct KeyboardEvents {}

impl KeyboardEvents {
    pub const fn new() -> Self {
        Self {}
    }
}

/* pub struct KeyboardEvent {
    pub scancode: u32,
    pub state: ElementState,
    pub virtual_keycode: Option<VirtualKeyCode>,
    pub modifiers: Modifiers,
}

pub enum ElementState {
    Press,
    Release,
} */

//

impl Stream for KeyboardEvents {
    type Item = char;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        wait_keyboard_event(cx).map(Some)
    }
}
