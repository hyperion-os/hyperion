use super::task::Task;
use alloc::sync::Arc;
use core::future::Future;
use crossbeam_queue::SegQueue;
use spin::Lazy;

//

static EXECUTOR: Lazy<Arc<Executor>> = Lazy::new(|| Arc::new(Executor::new()));

//

pub fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
    Task::spawn(EXECUTOR.clone(), fut);
}

pub fn run_tasks() -> ! {
    loop {
        EXECUTOR.run();
        // arch::wait_interrupt();
    }
}

//

#[derive(Default)]
pub struct Executor {
    tasks: SegQueue<Arc<Task>>,
}

impl Executor {
    pub const fn new() -> Self {
        Self {
            tasks: SegQueue::new(),
        }
    }

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
}
