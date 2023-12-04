use alloc::sync::Arc;
use core::{
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use crate::{futex, lock::Mutex, process, task::Pid};

//

pub fn pipe() -> (Sender<u8>, Receiver<u8>) {
    Pipe::new_pipe().split()
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Closed;

//

#[derive(Clone)]
pub struct Sender<T> {
    inner: Arc<Channel<T>>,
}

impl<T> Sender<T> {
    pub fn send(&self, item: T) -> Result<(), Closed> {
        self.inner.send(item)
    }

    pub fn close(&self) {
        self.inner.close_send()
    }
}

impl<T: Copy> Sender<T> {
    pub fn send_slice(&self, data: &[T]) -> Result<(), Closed> {
        self.inner.send_slice(data)
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.close()
    }
}

//

#[derive(Clone)]
pub struct Receiver<T> {
    inner: Arc<Channel<T>>,
}

impl<T> Receiver<T> {
    pub fn recv(&self) -> Result<T, Closed> {
        self.inner.recv()
    }

    pub fn close(&self) {
        self.inner.close_recv()
    }
}

impl<T: Copy> Receiver<T> {
    pub fn recv_slice(&self, buf: &mut [T]) -> Result<usize, Closed> {
        self.inner.recv_slice(buf)
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.close()
    }
}

//

pub type Pipe = Channel<u8>;

impl Pipe {
    pub fn new_pipe() -> Self {
        Self::new(0x1000)
    }
}

impl Default for Pipe {
    fn default() -> Self {
        Self::new_pipe()
    }
}

//

/// simple P2P 2-copy IPC channel
pub struct Channel<T> {
    /// the actual data channel
    pub send: Mutex<ringbuf::HeapProducer<T>>,
    pub recv: Mutex<ringbuf::HeapConsumer<T>>,

    pub send_closed: AtomicBool,
    pub recv_closed: AtomicBool,

    pub n_send: AtomicUsize,
    pub n_recv: AtomicUsize,
}

impl<T> Channel<T> {
    pub fn new(capacity: usize) -> Self {
        // TODO: custom allocator
        let (send, recv) = ringbuf::HeapRb::new(capacity).split();
        let (send, recv) = (Mutex::new(send), Mutex::new(recv));

        Self {
            send,
            recv,

            send_closed: AtomicBool::new(false),
            recv_closed: AtomicBool::new(false),

            n_send: AtomicUsize::new(0),
            n_recv: AtomicUsize::new(0),
        }
    }

    pub fn split(self) -> (Sender<T>, Receiver<T>) {
        let ch = Arc::new(self);
        (Sender { inner: ch.clone() }, Receiver { inner: ch })
    }

    pub fn send(&self, mut item: T) -> Result<(), Closed> {
        let mut stream = self.send.lock();
        loop {
            let n_recv = self.n_recv.load(Ordering::Acquire);
            let closed = self.recv_closed.load(Ordering::Acquire);
            if let Err(overflow) = stream.push(item) {
                if closed {
                    return Err(Closed);
                }

                // wake up a reader
                futex::wake(NonNull::from(&self.n_send), 1);

                // sleep with the send stream lock
                futex::wait(NonNull::from(&self.n_recv), n_recv);

                // keep trying to send the item
                item = overflow;
            } else {
                self.n_send.fetch_add(1, Ordering::Release);

                // wake up a reader
                futex::wake(NonNull::from(&self.n_send), 1);

                return Ok(());
            };
        }
    }

    pub fn recv(&self) -> Result<T, Closed> {
        let mut stream = self.recv.lock();
        loop {
            let n_send = self.n_send.load(Ordering::Acquire);
            let closed = self.send_closed.load(Ordering::Acquire);
            if let Some(item) = stream.pop() {
                self.n_recv.fetch_add(1, Ordering::Release);

                // wake up a sender
                futex::wake(NonNull::from(&self.n_recv), 1);

                return Ok(item);
            } else {
                if closed {
                    return Err(Closed);
                }

                // wake up a sender
                futex::wake(NonNull::from(&self.n_recv), 1);

                // sleep with the recv stream lock
                futex::wait(NonNull::from(&self.n_send), n_send);
            }
        }
    }

    fn close_send(&self) {
        self.n_send.fetch_add(1, Ordering::Release);
        self.send_closed.store(true, Ordering::Release);
        futex::wake(NonNull::from(&self.n_send), usize::MAX);
    }

    fn close_recv(&self) {
        self.n_recv.fetch_add(1, Ordering::Release);
        self.recv_closed.store(true, Ordering::Release);
        futex::wake(NonNull::from(&self.n_recv), usize::MAX);
    }
}

impl<T> Channel<T>
where
    T: Copy,
{
    pub fn send_slice(&self, data: &[T]) -> Result<(), Closed> {
        if data.is_empty() {
            return Ok(());
        }

        let mut stream = self.send.lock();
        let mut data = data;
        loop {
            let n_recv = self.n_recv.load(Ordering::Acquire);
            let closed = self.recv_closed.load(Ordering::Acquire);
            let sent = stream.push_slice(data);
            data = &data[sent..];

            self.n_send.fetch_add(sent, Ordering::Release);

            // wake up a reader
            futex::wake(NonNull::from(&self.n_send), 1);

            if closed {
                return Err(Closed);
            }

            if data.is_empty() {
                return Ok(());
            }

            // sleep with the send stream lock
            futex::wait(NonNull::from(&self.n_recv), n_recv);
        }
    }

    pub fn recv_slice(&self, buf: &mut [T]) -> Result<usize, Closed> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut stream = self.recv.lock();
        loop {
            let n_send = self.n_send.load(Ordering::Acquire);
            let closed = self.send_closed.load(Ordering::Acquire);
            let count = stream.pop_slice(buf);

            self.n_recv.fetch_add(count, Ordering::Release);

            // wake up a sender
            futex::wake(NonNull::from(&self.n_recv), 1);

            if count != 0 {
                return Ok(count);
            }

            if closed {
                return Err(Closed);
            }

            // sleep with the recv stream lock
            futex::wait(NonNull::from(&self.n_send), n_send);
        }
    }
}

//

pub fn send(target_pid: Pid, data: &[u8]) -> Result<(), &'static str> {
    target_pid
        .find()
        .ok_or("no such process")?
        .simple_ipc
        .send_slice(data)
        .map_err(|_| "stream closed")?;
    Ok(())
}

pub fn recv(buf: &mut [u8]) -> Result<usize, &'static str> {
    process()
        .simple_ipc
        .recv_slice(buf)
        .map_err(|_| "stream closed")
}
