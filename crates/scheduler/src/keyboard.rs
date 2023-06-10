use core::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Stream;
use hyperion_keyboard::{KEYBOARD_EVENT_QUEUE, KEYBOARD_EVENT_WAKER};

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

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<Self::Item>> {
        let queue = KEYBOARD_EVENT_QUEUE.get_force();

        if let Some(ev) = queue.pop() {
            return Poll::Ready(Some(ev));
        }

        // need to check if a keyboard event happened right here

        KEYBOARD_EVENT_WAKER.register(ctx.waker());

        // .. with this
        if let Some(ev) = queue.pop() {
            KEYBOARD_EVENT_WAKER.take();
            Poll::Ready(Some(ev))
        } else {
            Poll::Pending
        }
    }
}
