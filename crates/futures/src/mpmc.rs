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
    let inner = Arc::new(SplitChannel::new());
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
    inner: Arc<SplitChannel<T>>,
}

impl<T> Sender<T> {
    pub fn send(&self, data: T) -> Option<()> {
        self.inner.send(data)
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        if self.inner.writers.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.inner.channel.waker.wake();
        }
    }
}

//

#[derive(Clone)]
pub struct Receiver<T> {
    inner: Arc<SplitChannel<T>>,
}

impl<T> Receiver<T> {
    pub fn recv(&self) -> Recv<T> {
        self.inner.recv()
    }

    pub fn try_recv(&self) -> Option<T> {
        self.inner.try_recv()
    }

    pub fn race_stream(&self) -> RecvStream<T> {
        self.inner.race_stream()
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        if self.inner.readers.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.inner.channel.waker.wake();
        }
    }
}

//

pub struct Channel<T> {
    queue: SegQueue<T>,
    waker: AtomicWaker,
}

impl<T> Channel<T> {
    pub const fn new() -> Self {
        Self {
            queue: SegQueue::new(),
            waker: AtomicWaker::new(),
        }
    }

    pub fn send(&self, val: T) {
        self.queue.push(val);
        self.waker.wake();
    }

    pub fn recv(&self) -> ChannelRecv<T> {
        ChannelRecv { inner: self }
    }

    pub fn try_recv(&self) -> Option<T> {
        self.queue.pop()
    }

    pub fn race_stream(&self) -> ChannelRecvStream<T> {
        ChannelRecvStream { inner: self }
    }
}

//

pub struct ChannelRecv<'a, T> {
    inner: &'a Channel<T>,
}

impl<'a, T> Future for ChannelRecv<'a, T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Some(v) = self.inner.try_recv() {
            return Poll::Ready(Some(v));
        }

        self.inner.waker.register(cx.waker());

        if let Some(v) = self.inner.try_recv() {
            self.inner.waker.take();
            return Poll::Ready(Some(v));
        }

        Poll::Pending
    }
}

pub struct ChannelRecvStream<'a, T> {
    inner: &'a Channel<T>,
}

impl<'a, T> Stream for ChannelRecvStream<'a, T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.recv().poll_unpin(cx)
    }
}

pub struct Recv<'a, T> {
    inner: &'a SplitChannel<T>,
}

impl<'a, T> Future for Recv<'a, T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let closed = self.inner.writers.load(Ordering::SeqCst) == 0;

        if let Poll::Ready(val) = self.inner.channel.recv().poll_unpin(cx) {
            return Poll::Ready(val);
        }

        if closed {
            self.inner.channel.waker.take();
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

pub struct RecvStream<'a, T> {
    inner: &'a SplitChannel<T>,
}

impl<'a, T> Stream for RecvStream<'a, T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.recv().poll_unpin(cx)
    }
}

//

struct SplitChannel<T> {
    readers: AtomicUsize,
    writers: AtomicUsize,
    channel: Channel<T>,
}

impl<T> SplitChannel<T> {
    const fn new() -> Self {
        Self {
            readers: AtomicUsize::new(1),
            writers: AtomicUsize::new(1),
            channel: Channel::new(),
        }
    }

    fn send(&self, val: T) -> Option<()> {
        if self.readers.load(Ordering::SeqCst) == 0 {
            return None;
        }

        self.channel.send(val);
        Some(())
    }

    pub fn recv(&self) -> Recv<T> {
        Recv { inner: self }
    }

    pub fn try_recv(&self) -> Option<T> {
        self.channel.try_recv()
    }

    pub fn race_stream(&self) -> RecvStream<T> {
        RecvStream { inner: self }
    }
}
