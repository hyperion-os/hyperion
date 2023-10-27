use alloc::sync::Arc;
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
};

use crossbeam_queue::SegQueue;
use futures_util::{task::AtomicWaker, FutureExt, Stream};

//

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Channel::new());
    (
        Sender {
            inner: inner.clone(),
        },
        Receiver { inner },
    )
}

//

#[derive(Clone)]
pub struct Sender<T> {
    inner: Arc<Channel<T>>,
}

impl<T> Sender<T> {
    pub fn send(&self, data: T) -> Option<()> {
        if self.inner.readers.load(Ordering::SeqCst) == 0 {
            return None;
        }

        self.inner.queue.push(data);
        self.inner.waker.wake();

        Some(())
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.inner.writers.fetch_sub(1, Ordering::SeqCst);
    }
}

//

#[derive(Clone)]
pub struct Receiver<T> {
    inner: Arc<Channel<T>>,
}

impl<T> Receiver<T> {
    pub fn recv(&self) -> Recv<T> {
        Recv { inner: &self.inner }
    }

    pub fn try_recv(&self) -> Option<T> {
        self.inner.queue.pop()
    }

    pub fn race_stream(&self) -> RecvStream<T> {
        RecvStream { inner: &self.inner }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.inner.readers.fetch_sub(1, Ordering::SeqCst);
    }
}

//

pub struct Recv<'a, T> {
    inner: &'a Channel<T>,
}

impl<'a, T> Future for Recv<'a, T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let closed = self.inner.writers.load(Ordering::SeqCst) == 0;

        if let Some(v) = self.inner.queue.pop() {
            return Poll::Ready(Some(v));
        }

        if closed {
            return Poll::Ready(None);
        }

        self.inner.waker.register(cx.waker());

        if let Some(v) = self.inner.queue.pop() {
            self.inner.waker.take();
            return Poll::Ready(Some(v));
        }

        if closed {
            return Poll::Ready(None);
        }

        Poll::Pending
    }
}

pub struct RecvStream<'a, T> {
    inner: &'a Channel<T>,
}

impl<'a, T> Stream for RecvStream<'a, T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Recv { inner: self.inner }.poll_unpin(cx)
    }
}

//

struct Channel<T> {
    readers: AtomicUsize,
    writers: AtomicUsize,
    queue: SegQueue<T>,
    waker: AtomicWaker,
}

impl<T> Channel<T> {
    const fn new() -> Self {
        Self {
            readers: AtomicUsize::new(1),
            writers: AtomicUsize::new(1),
            queue: SegQueue::new(),
            waker: AtomicWaker::new(),
        }
    }
}
