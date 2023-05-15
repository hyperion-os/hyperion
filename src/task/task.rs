use alloc::boxed::Box;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use futures_util::Future;

use super::executor::Executor;

//

pub struct Task {
    pub id: TaskId,
    fut: Pin<Box<dyn Future<Output = ()>>>,
}

pub trait IntoTask {
    fn into_task(self, executor: &Executor) -> Task;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TaskId(pub u8);

//

impl Task {
    pub fn new(id: TaskId, fut: impl Future<Output = ()> + 'static) -> Self {
        Self {
            id,
            fut: Box::pin(fut),
        }
    }

    pub fn poll(&mut self, ctx: &mut Context) -> Poll<()> {
        self.fut.as_mut().poll(ctx)
    }
}

impl<F: Future<Output = ()> + 'static> IntoTask for F {
    fn into_task(self, executor: &Executor) -> Task {
        Task::new(executor.next_task_id(), self)
    }
}

impl IntoTask for Task {
    fn into_task(self, _: &Executor) -> Task {
        self
    }
}

/* impl<F: Future<Output = ()> + 'static> From<F> for Task {
    fn from(value: F) -> Self {
        Self {
            id: TaskId::next(),
            fut: Box::pin(value),
        }
    }
} */
