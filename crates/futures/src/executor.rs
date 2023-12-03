use alloc::sync::Arc;
use core::future::Future;

use crossbeam_queue::SegQueue;
use hyperion_scheduler::{condvar::Condvar, lock::Mutex};

use super::task::Task;

//

pub fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
    push_task(Arc::new(Task::from_future(fut)))
}

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
    pop_task()?.poll();
    Some(())
}

pub fn push_task(task: Arc<Task>) {
    TASK_QUEUE.push(task);

    *EMPTY.0.lock() = false;
    EMPTY.1.notify_one();
}

pub fn pop_task() -> Option<Arc<Task>> {
    TASK_QUEUE.pop()
}

//

static TASK_QUEUE: SegQueue<Arc<Task>> = SegQueue::new();
static EMPTY: (Mutex<bool>, Condvar) = (Mutex::new(true), Condvar::new());
