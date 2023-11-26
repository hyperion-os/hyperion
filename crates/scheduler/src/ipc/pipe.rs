use alloc::sync::Arc;
use core::{
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{futex, lock::Mutex, process, task::Pid};

//

#[derive(Clone)]
pub struct Sender<T> {
    inner: Arc<Channel<T>>,
}

impl<T> Sender<T> {
    pub fn send(&self, item: T) {
        self.inner.send(item)
    }
}

impl<T: Copy> Sender<T> {
    pub fn send_slice(&self, data: &[T]) {
        self.inner.send_slice(data)
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
}

impl<T: Copy> Receiver<T> {
    pub fn recv_slice(&self, buf: &mut [T]) -> usize {
        self.inner.recv_slice(buf)
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

            n_send: AtomicUsize::new(0),
            n_recv: AtomicUsize::new(0),
        }
    }

    pub fn split(self) -> (Sender<T>, Receiver<T>) {
        let ch = Arc::new(self);
        (Sender { inner: ch.clone() }, Receiver { inner: ch })
    }

    pub fn send(&self, mut item: T) {
        let mut stream = self.send.lock();
        loop {
            let n_recv = self.n_recv.load(Ordering::Acquire);
            if let Err(overflow) = stream.push(item) {
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

                return;
            };
        }
    }

    pub fn recv(&self) -> T {
        let mut stream = self.recv.lock();
        loop {
            let n_send = self.n_send.load(Ordering::Acquire);
            if let Some(item) = stream.pop() {
                self.n_recv.fetch_add(1, Ordering::Release);

                // wake up a sender
                futex::wake(NonNull::from(&self.n_recv), 1);

                return item;
            } else {
                // wake up a sender
                futex::wake(NonNull::from(&self.n_recv), 1);

                // sleep with the recv stream lock
                futex::wait(NonNull::from(&self.n_send), n_send);
            }
        }
    }
}

impl<T> Channel<T>
where
    T: Copy,
{
    pub fn send_slice(&self, data: &[T]) {
        if data.is_empty() {
            return;
        }

        let mut stream = self.send.lock();
        let mut data = data;
        loop {
            let n_recv = self.n_recv.load(Ordering::Acquire);
            let sent = stream.push_slice(data);
            data = &data[sent..];

            self.n_send.fetch_add(sent, Ordering::Release);

            // wake up a reader
            futex::wake(NonNull::from(&self.n_send), 1);

            if data.is_empty() {
                // if not full
                return;
            }

            // sleep with the send stream lock
            futex::wait(NonNull::from(&self.n_recv), n_recv);
        }
    }

    pub fn recv_slice(&self, buf: &mut [T]) -> usize {
        if buf.is_empty() {
            return 0;
        }

        let mut stream = self.recv.lock();
        loop {
            let n_send = self.n_send.load(Ordering::Acquire);
            let count = stream.pop_slice(buf);

            self.n_recv.fetch_add(count, Ordering::Release);

            // wake up a sender
            futex::wake(NonNull::from(&self.n_recv), 1);

            if count != 0 {
                return count;
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
        .send_slice(data);
    Ok(())
}

pub fn recv(buf: &mut [u8]) -> usize {
    process().simple_ipc.recv_slice(buf)
}
