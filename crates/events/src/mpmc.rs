use core::{
    fmt::Debug,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use event_listener::{Event, EventListener};
use futures_util::ready;
use heapless::mpmc::MpMcQueue;
use pin_project::pin_project;

//

pub(crate) struct EventQueue<T> {
    queue: MpMcQueue<T, 128>,
    ops: Event,
}

impl<T: Debug> EventQueue<T> {
    pub const fn new() -> Self {
        Self {
            queue: MpMcQueue::new(),
            ops: Event::new(),
        }
    }

    pub fn send(&self, event: T) {
        _ = self.queue.enqueue(event);
        self.ops.notify(1);
    }

    pub fn try_recv(&self) -> Option<T> {
        self.queue.dequeue()
    }

    pub const fn recv(&self) -> Recv<T> {
        Recv {
            queue: self,
            slow: None,
        }
    }
}

//

#[must_use]
#[pin_project]
pub struct Recv<'a, T> {
    queue: &'a EventQueue<T>,
    #[pin]
    slow: Option<RecvSlow>,
}

impl<T: Debug> Future for Recv<'_, T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let this = self.project();
        let mut slow: Pin<&mut Option<RecvSlow>> = this.slow;

        let slow = if let Some(slow) = slow.as_mut().as_pin_mut() {
            slow
        } else if let Some(ev) = this.queue.try_recv() {
            return Poll::Ready(ev);
        } else {
            slow.set(Some(RecvSlow {
                listener: EventListener::new(),
            }));
            slow.as_pin_mut().unwrap()
        };

        slow.poll(this.queue, cx)
    }
}

#[must_use]
#[pin_project]
struct RecvSlow {
    #[pin]
    listener: EventListener,
}

impl RecvSlow {
    #[cold]
    fn poll<T: Debug>(self: Pin<&mut Self>, queue: &EventQueue<T>, cx: &mut Context) -> Poll<T> {
        let this = self.project();
        let mut listener: Pin<&mut EventListener> = this.listener;

        loop {
            if !listener.is_listening() {
                listener.as_mut().listen(&queue.ops);

                if let Some(ev) = queue.try_recv() {
                    return Poll::Ready(ev);
                }
            } else {
                ready!(listener.as_mut().poll(cx));

                if let Some(ev) = queue.try_recv() {
                    return Poll::Ready(ev);
                }
            }
        }
    }
}
