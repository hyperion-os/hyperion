use core::{future::Future, pin::Pin, task::Context};

use alloc::{boxed::Box, sync::Arc};
use futures::{
    future::{FusedFuture, FutureExt},
    task::ArcWake,
};
use spin::Mutex;

//

pub struct Task(Arc<TaskInner>);

impl Task {
    pub fn new(fut: impl Future<Output = ()> + Send + 'static) -> Self {
        let inner = Arc::new(TaskInner {
            fut: Mutex::new(Box::pin(fut.fuse())),
        });

        Self(inner as _)
    }

    pub fn poll(&self) {
        let mut fut = self.0.fut.lock();
        let fut = &mut *fut;

        if fut.is_terminated() {
            return;
        }

        let waker = futures::task::waker_ref(&self.0);
        let mut cx = Context::from_waker(&waker);

        _ = fut.as_mut().poll(&mut cx);
    }
}

impl<F: Future<Output = ()> + Send + 'static> From<F> for Task {
    fn from(value: F) -> Self {
        Self::new(value)
    }
}

impl ArcWake for TaskInner {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        crate::spawn(Task(arc_self.clone()));
    }
}

//

struct TaskInner {
    fut: Mutex<Pin<Box<dyn FusedFuture<Output = ()> + Send>>>,
}
