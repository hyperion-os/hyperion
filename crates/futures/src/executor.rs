use core::future::IntoFuture;

use crossbeam_queue::SegQueue;
use hyperion_scheduler::{condvar::Condvar, lock::Mutex};

use crate::task::{JoinHandle, Task};

//

// spawn a new task
pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: IntoFuture,
    F::IntoFuture: Send + 'static,
    F::Output: Send + 'static,
{
    JoinHandle::spawn(fut)
}

// execute tasks forever
pub fn run_tasks() -> ! {
    loop {
        while run_once().is_some() {}

        let mut empty = EMPTY.0.lock();
        *empty = TASK_QUEUE.is_empty();
        while *empty {
            empty = EMPTY.1.wait(empty);
        }
    }
}

pub fn run_once() -> Option<()> {
    _ = pop_task()?.poll();
    Some(())
}

pub(crate) fn push_task(task: Task) {
    TASK_QUEUE.push(task);

    *EMPTY.0.lock() = false;
    EMPTY.1.notify_one();
}

pub(crate) fn pop_task() -> Option<Task> {
    TASK_QUEUE.pop()
}

//

static TASK_QUEUE: SegQueue<Task> = SegQueue::new();
static EMPTY: (Mutex<bool>, Condvar) = (Mutex::new(true), Condvar::new());
