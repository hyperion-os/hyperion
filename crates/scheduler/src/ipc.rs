use alloc::borrow::Cow;

use crossbeam_queue::SegQueue;

use crate::{
    cleanup::Cleanup,
    process,
    task::{switch_because, Pid, Task, TaskState, PROCESSES},
    wait_next_task, READY,
};

//

/// simple P2P 2-copy IPC channel
#[derive(Debug, Default)]
pub struct SimpleIpc {
    /// the actual data channel
    pub channel: SegQueue<Cow<'static, [u8]>>,

    /// task waiting list when the channel is empty and processes are reading from it
    pub waiting: SegQueue<Task>,
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
    let proc = process();

    loop {
        if let Some(data) = proc.simple_ipc.channel.pop() {
            return data;
        }

        let mut data = None; // data while waiting for the next task
        let Some(next) = wait_next_task(|| {
            data = proc.simple_ipc.channel.pop();
            data.is_some()
        }) else {
            return data.unwrap();
        };

        // start waiting for events on the channel
        switch_because(next, TaskState::Sleeping, Cleanup::SimpleIpcWait);
    }
}
