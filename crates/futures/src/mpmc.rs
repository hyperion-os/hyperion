use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};

use crossbeam_queue::SegQueue;
use event_listener::Event;
use futures::stream::{unfold, Stream};

use crate::block_on;

//

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    Channel::new().split()
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendError<T>(pub T);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecvError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryRecvError {
    Empty,
    Closed,
}

//

pub struct Sender<T> {
    inner: Arc<SplitChannel<T>>,
}

impl<T> Sender<T> {
    pub fn send(&self, data: T) -> Result<(), SendError<T>> {
        self.inner.send(data)
    }

    pub fn receiver(&self) -> Option<Receiver<T>> {
        loop {
            let current = self.inner.readers.load(Ordering::Acquire);
            if current == 0 {
                // the read end was closed
                return None;
            }

            // fetch_add but don't increment if it was 0
            // once the count goes to 0, the channel is permanently closed
            if self
                .inner
                .readers
                .compare_exchange(current, current + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        Some(Receiver {
            inner: self.inner.clone(),
        })
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.inner.writers.fetch_add(1, Ordering::Acquire);
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.inner.close_send();
    }
}

//

pub struct Receiver<T> {
    inner: Arc<SplitChannel<T>>,
}

impl<T> Receiver<T> {
    pub async fn recv(&self) -> Result<T, RecvError> {
        self.inner.recv().await
    }

    pub fn blocking_recv(&self) -> Result<T, RecvError> {
        self.inner.blocking_recv()
    }

    pub fn spin_recv(&self) -> Result<T, RecvError> {
        self.inner.spin_recv()
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.inner.try_recv()
    }

    pub fn race_stream(&self) -> impl Stream<Item = T> + Unpin + '_ {
        self.inner.race_stream()
    }

    // F: FnMut(T) -> Fut,
    // Fut: Future<Output = Option<(Item, T)>>,
}

impl<T: Send> Receiver<T> {
    pub fn into_stream(self) -> impl Stream<Item = T> + Send + Unpin {
        unfold(self, |ch| {
            // TODO: same as [`Channel::race_stream`]
            Box::pin(async move {
                let item = Box::pin(ch.recv()).await.ok()?;
                Some((item, ch))
            })
        })
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        self.inner.readers.fetch_add(1, Ordering::Acquire);
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.inner.close_recv();
    }
}

//

pub struct Channel<T> {
    queue: SegQueue<T>,
    wakers: Event,
}

impl<T> Channel<T> {
    pub const fn new() -> Self {
        Self {
            queue: SegQueue::new(),
            wakers: Event::new(),
        }
    }

    pub fn split(self) -> (Sender<T>, Receiver<T>) {
        let inner = Arc::new(SplitChannel::new(self));
        let tx = Sender {
            inner: inner.clone(),
        };
        let rx = Receiver { inner };

        (tx, rx)
    }

    pub fn send(&self, val: T) {
        self.queue.push(val);
        self.wakers.notify(1);
    }

    pub fn try_recv(&self) -> Option<T> {
        self.queue.pop()
    }

    pub async fn recv(&self) -> T {
        if let Some(val) = self.try_recv() {
            return val;
        }

        self.recv_slow().await
    }

    pub fn race_stream(&self) -> impl Stream<Item = T> + Unpin + '_ {
        unfold(self, |ch| {
            // TODO: manual Future impl to get rid of the Box::pin
            Box::pin(async move {
                let item = ch.recv().await;
                Some((item, ch))
            })
        })
    }

    pub fn into_stream(self) -> impl Stream<Item = T> + Unpin {
        unfold(self, |ch| {
            // TODO: manual Future impl to get rid of the Box::pin
            Box::pin(async move {
                let item = ch.recv().await;
                Some((item, ch))
            })
        })
    }

    #[cold]
    async fn recv_slow(&self) -> T {
        loop {
            let l = self.wakers.listen();

            if let Some(val) = self.try_recv() {
                return val;
            }

            l.await;

            if let Some(val) = self.try_recv() {
                return val;
            }
        }
    }
}

//

struct SplitChannel<T> {
    readers: AtomicUsize,
    writers: AtomicUsize,
    channel: Channel<T>,
}

impl<T> SplitChannel<T> {
    const fn new(channel: Channel<T>) -> Self {
        Self {
            readers: AtomicUsize::new(1),
            writers: AtomicUsize::new(1),
            channel,
        }
    }

    fn close_send(&self) {
        if self.writers.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.channel.wakers.notify(usize::MAX);
        }
    }

    fn close_recv(&self) {
        if self.readers.fetch_sub(1, Ordering::SeqCst) == 1 {
            // self.channel.wakers.notify(usize::MAX);
        }
    }

    fn is_send_closed(&self) -> bool {
        self.writers.load(Ordering::SeqCst) == 0
    }

    fn is_recv_closed(&self) -> bool {
        self.readers.load(Ordering::SeqCst) == 0
    }

    fn send(&self, val: T) -> Result<(), SendError<T>> {
        if self.is_recv_closed() {
            return Err(SendError(val));
        }

        self.channel.send(val);
        Ok(())
    }

    async fn recv(&self) -> Result<T, RecvError> {
        match self.try_recv() {
            Ok(val) => return Ok(val),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Closed) => return Err(RecvError),
        }

        self.recv_slow().await
    }

    fn blocking_recv(&self) -> Result<T, RecvError> {
        block_on(self.recv())
    }

    fn spin_recv(&self) -> Result<T, RecvError> {
        loop {
            match self.try_recv() {
                Ok(val) => return Ok(val),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Closed) => return Err(RecvError),
            }
        }
    }

    fn try_recv(&self) -> Result<T, TryRecvError> {
        if let Some(val) = self.channel.try_recv() {
            return Ok(val);
        }

        if self.is_send_closed() {
            return Err(TryRecvError::Closed);
        }

        Err(TryRecvError::Empty)
    }

    fn race_stream(&self) -> impl Stream<Item = T> + Unpin + '_ {
        unfold(self, |ch| {
            // TODO: same as [`Channel::race_stream`]
            Box::pin(async move {
                let item = Box::pin(ch.recv()).await.ok()?;
                Some((item, ch))
            })
        })
    }

    #[cold]
    async fn recv_slow(&self) -> Result<T, RecvError> {
        loop {
            let l = self.channel.wakers.listen();

            match self.try_recv() {
                Ok(val) => return Ok(val),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Closed) => return Err(RecvError),
            }

            l.await;

            match self.try_recv() {
                Ok(val) => return Ok(val),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Closed) => return Err(RecvError),
            }
        }
    }
}
