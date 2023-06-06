use core::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{task::AtomicWaker, Stream};

//

pub static WAKER: AtomicWaker = AtomicWaker::new();

//

pub fn provide_tick() {
    WAKER.wake()
}

//

#[derive(Debug, Clone, Copy, Default)]
pub struct Ticks {
    waiting: bool,
}

impl Ticks {
    pub const fn new() -> Self {
        Self { waiting: false }
    }
}

impl Stream for Ticks {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<Self::Item>> {
        let was_waiting = self.waiting;
        self.waiting = !self.waiting;
        if was_waiting {
            return Poll::Ready(Some(()));
        }

        WAKER.register(ctx.waker());
        Poll::Pending
    }
}
