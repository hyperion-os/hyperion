use alloc::sync::Arc;
use core::future::Future;

use crossbeam_queue::SegQueue;

use super::task::Task;
use crate::yield_now;

//

pub fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
    push_task(Arc::new(Task::from_future(fut)))
}

pub fn run_tasks() -> ! {
    loop {
        while run_once().is_some() {}
        yield_now();
    }
}

pub fn run_once() -> Option<()> {
    pop_task()?.poll();
    Some(())
}

pub fn push_task(task: Arc<Task>) {
    TASK_QUEUE.push(task)
}

pub fn pop_task() -> Option<Arc<Task>> {
    TASK_QUEUE.pop()
}

//

static TASK_QUEUE: SegQueue<Arc<Task>> = SegQueue::new();
