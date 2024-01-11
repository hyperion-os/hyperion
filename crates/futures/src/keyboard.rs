use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{ready, Stream};
use hyperion_events::{
    keyboard::{buffer::recv, event::KeyboardEvent},
    Recv,
};

//

pub const fn keyboard_events() -> KeyboardEvents {
    KeyboardEvents { inner: None }
}

//

#[must_use]
pub struct KeyboardEvents {
    inner: Option<Pin<Box<Recv<'static, KeyboardEvent>>>>,
}

//

impl Stream for KeyboardEvents {
    type Item = KeyboardEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let inner = self.get_mut().inner.get_or_insert_with(|| Box::pin(recv()));

        let ev = ready!(inner.as_mut().poll(cx));
        inner.set(recv()); // reset the Future
        Poll::Ready(Some(ev))
    }
}
