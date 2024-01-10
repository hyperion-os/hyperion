use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{ready, Stream};
use hyperion_input::{
    mouse::{buffer::recv, event::MouseEvent},
    Recv,
};

//

pub const fn mouse_events() -> MouseEvents {
    MouseEvents { inner: None }
}

//

#[must_use]
pub struct MouseEvents {
    inner: Option<Pin<Box<Recv<'static, MouseEvent>>>>,
}

//

impl Stream for MouseEvents {
    type Item = MouseEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let inner = self.get_mut().inner.get_or_insert_with(|| Box::pin(recv()));

        let ev = ready!(inner.as_mut().poll(cx));
        inner.set(recv()); // reset the Future
        Poll::Ready(Some(ev))
    }
}
