use core::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Stream;
use hyperion_keyboard::{event::KeyboardEvent, wait_keyboard_event};

//

pub const fn keyboard_events() -> KeyboardEvents {
    KeyboardEvents
}

//

#[derive(Debug, Clone, Copy, Default)]
#[must_use]
pub struct KeyboardEvents;

//

impl Stream for KeyboardEvents {
    type Item = KeyboardEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        wait_keyboard_event(cx).map(Some)
    }
}
