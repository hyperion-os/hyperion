use crossbeam_queue::SegQueue;
use hyperion_scheduler::{condvar::Condvar, lock::Mutex};

use crate::task::{IntoTask, Task};

//

// spawn a new task
pub fn spawn(task: impl IntoTask) {
    push_task(task.into_task())
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
    pop_task()?.poll();
    Some(())
}

fn push_task(task: Task) {
    TASK_QUEUE.push(task);

    *EMPTY.0.lock() = false;
    EMPTY.1.notify_one();
}

fn pop_task() -> Option<Task> {
    TASK_QUEUE.pop()
}

//

static TASK_QUEUE: SegQueue<Task> = SegQueue::new();
static EMPTY: (Mutex<bool>, Condvar) = (Mutex::new(true), Condvar::new());
