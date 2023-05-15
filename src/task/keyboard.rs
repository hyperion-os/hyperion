use crate::{debug, warn};
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use crossbeam_queue::ArrayQueue;
use futures_util::{task::AtomicWaker, Stream};
use spin::Lazy;

//

pub static KEYBOARD_EVENT_Q: Lazy<ArrayQueue<char>> = Lazy::new(|| ArrayQueue::new(256));
pub static WAKER: AtomicWaker = AtomicWaker::new();

//

pub fn provide_keyboard_event(c: char) {
    if let Some(old) = KEYBOARD_EVENT_Q.force_push(c) {
        warn!("Keyboard event queue full! Lost '{old}'");
    }
    WAKER.wake()
}

//

#[derive(Debug, Clone, Copy, Default)]
pub struct KeyboardEvents {}

impl KeyboardEvents {
    pub fn new() -> Self {
        Self::default()
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
        if let Some(ev) = KEYBOARD_EVENT_Q.pop() {
            return Poll::Ready(Some(ev));
        }

        // need to check if a keyboard event happened right here

        WAKER.register(ctx.waker());

        // .. with this
        if let Some(ev) = KEYBOARD_EVENT_Q.pop() {
            WAKER.take();
            Poll::Ready(Some(ev))
        } else {
            Poll::Pending
        }
    }
}
