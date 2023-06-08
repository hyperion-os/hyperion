use core::{
    pin::Pin,
    task::{Context, Poll},
};

use crossbeam_queue::ArrayQueue;
use futures_util::{task::AtomicWaker, Stream};
use hyperion_log::warn;

use crate::util::int_safe_lazy::IntSafeLazy;

//

pub static KEYBOARD_EVENT_Q: IntSafeLazy<ArrayQueue<char>> =
    IntSafeLazy::new(|| ArrayQueue::new(256));
pub static WAKER: AtomicWaker = AtomicWaker::new();

//

pub fn provide_keyboard_event(c: char) {
    let Some(queue) = KEYBOARD_EVENT_Q.get() else {
        return
    };

    if let Some(old) = queue.force_push(c) {
        warn!("Keyboard event queue full! Lost '{old}'");
    }

    WAKER.wake()
}

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
        let queue = KEYBOARD_EVENT_Q.get_force();

        if let Some(ev) = queue.pop() {
            return Poll::Ready(Some(ev));
        }

        // need to check if a keyboard event happened right here

        WAKER.register(ctx.waker());

        // .. with this
        if let Some(ev) = queue.pop() {
            WAKER.take();
            Poll::Ready(Some(ev))
        } else {
            Poll::Pending
        }
    }
}
