use alloc::borrow::Cow;

use crossbeam_queue::{ArrayQueue, SegQueue};

use crate::{
    cleanup::Cleanup,
    process,
    task::{switch_because, Pid, Process, Task, TaskState, PROCESSES},
    wait_next_task, READY,
};

//

/// simple P2P 2-copy IPC channel
#[derive(Debug)]
pub struct SimpleIpc {
    /// the latest half consumed chunk of data
    pub tail: ArrayQueue<Cow<'static, [u8]>>,

    /// the actual data channel
    pub channel: SegQueue<Cow<'static, [u8]>>,

    /// task waiting list when the channel is empty and processes are reading from it
    pub waiting: SegQueue<Task>,
}

impl SimpleIpc {
    pub fn new() -> Self {
        Self {
            tail: ArrayQueue::new(1),
            channel: SegQueue::new(),
            waiting: SegQueue::new(),
        }
    }
}

//

pub fn start_waiting(task: Task) {
    let proc = task.process.clone();

    if !proc.simple_ipc.channel.is_empty() {
        READY.push(task);
    } else {
        proc.simple_ipc.waiting.push(task);
    }
}

pub fn send(target_pid: Pid, data: Cow<'static, [u8]>) -> Result<(), &'static str> {
    let proc = PROCESSES
        .lock()
        .get(&target_pid)
        .and_then(|mem_weak_ref| mem_weak_ref.upgrade())
        .ok_or("no such process")?;

    proc.simple_ipc.channel.push(data);
    let recv_task = proc.simple_ipc.waiting.pop();

    if let Some(recv_task) = recv_task {
        // READY.push(recv_task);
        switch_because(recv_task, TaskState::Ready, Cleanup::Ready);
    }

    Ok(())
}

pub fn recv() -> Cow<'static, [u8]> {
    recv_with(&process())
}

pub fn recv_to(buf: &mut [u8]) {
    let proc = process();

    let data = recv_with(&proc);

    // limit buf to be at most the length of available data
    let buf = &mut buf[..data.len().min(data.len())];

    // fill the buf and send the rest to tail
    let (buf_data, left) = data.split_at(buf.len());

    // FIXME: multiple calls to read_to in the same process might cause data race problems
    proc.simple_ipc
        .tail
        .push(left.to_vec().into())
        .expect("FIXME: multi read_to data race");

    buf.copy_from_slice(buf_data);
}

fn recv_with(proc: &Process) -> Cow<'static, [u8]> {
    loop {
        if let Some(data) = try_recv_with(&proc) {
            return data;
        }

        let next = match wait_next_task(|| try_recv_with(&proc)) {
            Ok(task) => task,
            Err(data) => return data,
        };

        // start waiting for events on the channel
        switch_because(next, TaskState::Sleeping, Cleanup::SimpleIpcWait);
    }
}

fn try_recv_with(proc: &Process) -> Option<Cow<'static, [u8]>> {
    proc.simple_ipc.tail.pop()?;
    proc.simple_ipc.channel.pop()?;
    None
}
