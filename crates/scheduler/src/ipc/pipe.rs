use core::{
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{futex, lock::Mutex, process, task::Pid};

//

/// simple P2P 2-copy IPC channel
pub struct Pipe {
    /// the actual data channel
    pub send: Mutex<ringbuf::HeapProducer<u8>>,
    pub recv: Mutex<ringbuf::HeapConsumer<u8>>,

    pub items: AtomicUsize,
}

impl Pipe {
    pub fn new() -> Self {
        // TODO: custom allocator
        let (send, recv) = ringbuf::HeapRb::new(0x1000).split();
        let (send, recv) = (Mutex::new(send), Mutex::new(recv));

        Self {
            send,
            recv,

            items: AtomicUsize::new(0),
        }
    }
}

impl Default for Pipe {
    fn default() -> Self {
        Self::new()
    }
}

//

pub fn send(target_pid: Pid, data: &[u8]) -> Result<(), &'static str> {
    if data.is_empty() {
        return Ok(());
    }

    let proc = target_pid.find().ok_or("no such process")?;
    let pipe = &proc.simple_ipc;

    let mut stream = pipe.send.lock();
    let mut data = data;
    loop {
        if data.is_empty() {
            return Ok(());
        }

        let sent = stream.push_slice(data);
        data = &data[sent..];

        pipe.items.fetch_add(sent, Ordering::Release);

        // wake up a reader
        futex::wake(NonNull::from(&pipe.items), 1);

        // sleep with the send stream lock
        futex::wait(NonNull::from(&pipe.items), 0x1000);
    }
}

pub fn recv(buf: &mut [u8]) -> usize {
    if buf.is_empty() {
        return 0;
    }

    let proc = process();
    let pipe = &proc.simple_ipc;

    let mut stream = pipe.recv.lock();
    loop {
        let count = stream.pop_slice(buf);

        // can race and wrap from -1, but it doesn't matter
        pipe.items.fetch_sub(count, Ordering::Release);

        // wake up a sender
        futex::wake(NonNull::from(&pipe.items), 1);

        if count != 0 {
            return count;
        }

        // sleep with the recv stream lock
        futex::wait(NonNull::from(&pipe.items), 0);
    }
}
