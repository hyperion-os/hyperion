use alloc::sync::Arc;
use core::future::Future;

use crossbeam_queue::SegQueue;
use spin::Lazy;

use super::task::Task;

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

pub struct Executor {
    tasks: SegQueue<Arc<Task>>,
}

impl Executor {
    pub fn new() -> Self {
        // TODO:
        // crate::mem::force_init_allocator();
        Self {
            tasks: <_>::default(),
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

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}
