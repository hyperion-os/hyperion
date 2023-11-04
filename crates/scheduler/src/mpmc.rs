use alloc::sync::Arc;
use core::sync::atomic::Ordering;

use crossbeam_queue::SegQueue;

use crate::wait_next_task;

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
        self.inner.send(data)
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
    pub fn recv(&self) -> T {
        self.inner.recv()
    }

    pub fn try_recv(&self) -> Option<T> {
        self.inner.try_recv()
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

//

pub struct Channel<T> {
    queue: SegQueue<T>,
    waiting: SegQueue<Task>,
}

impl<T> Channel<T> {
    pub const fn new() -> Self {
        Self {
            queue: SegQueue::new(),
            waiting: SegQueue::new(),
        }
    }

    // blocking send
    pub fn send(&self, data: T) -> Option<()> {
        self.queue.push(data);

        if let Some(recv_task) = self.waiting.pop() {
            // READY.push(recv_task);
            switch_because(recv_task, TaskState::Ready, Cleanup::Ready);
        }

        Some(())
    }

    /// blocking recv
    pub fn recv(&self) -> T {
        loop {
            if let Some(data) = self.try_recv() {
                return data;
            }

            let next = match wait_next_task(|| self.try_recv()) {
                Ok(task) => task,
                Err(data) => return data,
            };

            // start waiting for events on the channel
            switch_because(next, TaskState::Sleeping, Cleanup::SimpleIpcWait);
        }
    }

    /// non-blocking recv
    pub fn try_recv(&self) -> Option<T> {
        self.queue.pop()
    }
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelClosed;
