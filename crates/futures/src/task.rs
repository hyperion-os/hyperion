use alloc::{boxed::Box, sync::Arc};
use core::{
    future::IntoFuture,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use futures_util::{
    task::{waker, ArcWake},
    Future,
};
use spin::Mutex;

use crate::executor;

//

pub struct Task(Arc<TaskInner>);

//

impl Task {
    pub fn new<F>(fut: F) -> Self
    where
        F: IntoFuture<Output = ()>,
        F::IntoFuture: Send + 'static,
    {
        Self::from_inner(TaskInner::new(Box::pin(fut.into_future())))
    }

    fn from_inner(inner: TaskInner) -> Self {
        Self(Arc::new(inner))
    }

    pub fn waker(self) -> Waker {
        waker(self.0)
    }

    pub fn wake(self) {
        executor::spawn(self)
    }

    pub fn poll(self) {
        let Some(mut future) = self.0.future.try_lock() else {
            // another CPU is already working on this task
            return;
        };

        let TaskFuture::Future(fut) = &mut *future else {
            // this future is already completed
            return;
        };

        let waker = self.clone().waker();
        let mut cx = Context::from_waker(&waker);

        if let Poll::Ready(result) = fut.as_mut().poll(&mut cx) {
            *future = TaskFuture::Result(result);
        }
    }
}

impl Clone for Task {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

//

pub struct TaskInner {
    future: Mutex<TaskFuture>,
}

impl TaskInner {
    pub fn new(fut: Pin<Box<dyn Future<Output = ()> + Send>>) -> Self {
        Self {
            future: Mutex::new(TaskFuture::Future(fut)),
        }
    }
}

impl ArcWake for TaskInner {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        Task(arc_self.clone()).wake();
    }
}

//

pub enum TaskFuture {
    /// A kernel task
    Future(Pin<Box<dyn Future<Output = ()> + Send>>),
    Result(()),
    // None,
}

//

pub trait IntoTask {
    fn into_task(self) -> Task;
}

// impl IntoTask for TaskInner {
//     fn into_task(self) -> Task {
//         Task::from_inner(self)
//     }
// }

impl IntoTask for Task {
    fn into_task(self) -> Task {
        self
    }
}

impl<F> IntoTask for F
where
    F: IntoFuture<Output = ()>,
    F::IntoFuture: Send + 'static,
{
    fn into_task(self) -> Task {
        Task::new(self)
    }
}
