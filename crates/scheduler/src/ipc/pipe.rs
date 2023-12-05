use alloc::sync::Arc;

use crate::{condvar::Condvar, lock::Mutex, process, task::Pid};

//

pub fn channel() -> (Sender<u8>, Receiver<u8>) {
    Pipe::new_pipe().split()
}

pub fn channel_with(capacity: usize) -> (Sender<u8>, Receiver<u8>) {
    Pipe::new(capacity).split()
}

pub fn pipe() -> Arc<Channel<u8>> {
    Arc::new(Pipe::new_pipe())
}

pub fn pipe_with(capacity: usize) -> Arc<Channel<u8>> {
    Arc::new(Pipe::new(capacity))
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

    pub fn wait_closed(&self) {
        self.inner.recv_closed();
    }

    pub fn close(&self) {
        self.inner.close_send()
    }
}

impl<T: Copy> Sender<T> {
    /// Sender doesn't keep the recv side open
    pub fn weak_recv_slice(&self, buf: &mut [T]) -> Result<usize, Closed> {
        self.inner.recv_slice(buf)
    }

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

    pub fn wait_closed(&self) {
        self.inner.send_closed();
    }

    pub fn close(&self) {
        self.inner.close_recv()
    }
}

impl<T: Copy> Receiver<T> {
    /// Receiver doesn't keep the send side open
    pub fn weak_send_slice(&self, data: &[T]) -> Result<(), Closed> {
        self.inner.send_slice(data)
    }

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

    pub send_wait: Condvar,
    pub recv_wait: Condvar,

    pub send_closed: Mutex<bool>,
    pub recv_closed: Mutex<bool>,
    // pub n_send: AtomicUsize,
    // pub n_recv: AtomicUsize,
}

impl<T> Channel<T> {
    pub fn new(capacity: usize) -> Self {
        // TODO: custom allocator
        let (send, recv) = ringbuf::HeapRb::new(capacity).split();
        let (send, recv) = (Mutex::new(send), Mutex::new(recv));

        Self {
            send,
            recv,

            send_wait: Condvar::new(),
            recv_wait: Condvar::new(),

            send_closed: Mutex::new(false),
            recv_closed: Mutex::new(false),
            // n_send: AtomicUsize::new(0),
            // n_recv: AtomicUsize::new(0),
        }
    }

    pub fn split(self) -> (Sender<T>, Receiver<T>) {
        let ch = Arc::new(self);
        (Sender { inner: ch.clone() }, Receiver { inner: ch })
    }

    pub fn send(&self, mut item: T) -> Result<(), Closed> {
        let mut stream = self.send.lock();
        let mut r_closed = self.recv_closed.lock();
        loop {
            if *r_closed {
                return Err(Closed);
            }

            if let Err(overflow) = stream.push(item) {
                self.send_wait.notify_one();
                r_closed = self.recv_wait.wait(r_closed);

                // keep trying to send the item
                item = overflow;
            } else {
                self.send_wait.notify_one();
                return Ok(());
            };
        }
    }

    pub fn recv(&self) -> Result<T, Closed> {
        let mut stream = self.recv.lock();
        let mut s_closed = self.send_closed.lock();
        loop {
            if let Some(item) = stream.pop() {
                self.recv_wait.notify_one();
                return Ok(item);
            } else {
                if *s_closed {
                    return Err(Closed);
                }

                self.recv_wait.notify_one();
                s_closed = self.send_wait.wait(s_closed);
            }
        }
    }

    /// wait for the sender to be closed
    pub fn send_closed(&self) {
        let mut s_closed = self.send_closed.lock();
        loop {
            if *s_closed {
                return;
            }

            s_closed = self.send_wait.wait(s_closed);
        }
    }

    /// wait for the receiver to be closed
    pub fn recv_closed(&self) {
        let mut r_closed = self.recv_closed.lock();
        loop {
            if *r_closed {
                return;
            }

            r_closed = self.recv_wait.wait(r_closed);
        }
    }

    fn close_send(&self) {
        *self.send_closed.lock() = true;
        self.send_wait.notify_one();
        self.recv_wait.notify_one();
    }

    fn close_recv(&self) {
        *self.recv_closed.lock() = true;
        self.send_wait.notify_one();
        self.recv_wait.notify_one();
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

        let mut data = data;

        let mut stream = self.send.lock();
        let mut r_closed = self.recv_closed.lock();
        loop {
            if *r_closed {
                return Err(Closed);
            }

            let sent = stream.push_slice(data);
            data = &data[sent..];

            self.send_wait.notify_one();

            if data.is_empty() {
                return Ok(());
            }

            r_closed = self.recv_wait.wait(r_closed);
        }
    }

    pub fn recv_slice(&self, buf: &mut [T]) -> Result<usize, Closed> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut stream = self.recv.lock();
        let mut s_closed = self.send_closed.lock();
        loop {
            let count = stream.pop_slice(buf);

            self.recv_wait.notify_one();

            if count != 0 {
                return Ok(count);
            }

            if *s_closed {
                return Err(Closed);
            }

            s_closed = self.send_wait.wait(s_closed);
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
