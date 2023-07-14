use alloc::sync::Arc;
use core::future::Future;

use crossbeam_queue::SegQueue;

use super::task::Task;

//

pub fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
    push_task(Arc::new(Task::from_future(fut)))
}

pub fn run_tasks() -> ! {
    loop {
        while run_once().is_some() {}
        // arch::wait_interrupt();
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

/* pub struct Executor {
    tasks: SegQueue<Arc<Task>>,
}

impl Executor {
    pub fn add_task(&self, task: Arc<Task>) {
        self.tasks.push(task)
    }

    pub fn take_task(&self) -> Option<Arc<Task>> {
        self.tasks.pop()
    }

    pub fn run(&self) {
        while let Some(task) = self.take_task() {
            task.poll();
        }
    }
} */

//

static TASK_QUEUE: SegQueue<Arc<Task>> = SegQueue::new();
