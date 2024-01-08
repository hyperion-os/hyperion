use alloc::{boxed::Box, sync::Arc};
use core::{
    future::IntoFuture,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use futures_util::{
    task::{waker, ArcWake, AtomicWaker},
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
        executor::push_task(self);
    }

    pub fn poll(self) -> Poll<()> {
        let Some(mut future) = self.0.future.try_lock() else {
            // another CPU is already working on this task
            return Poll::Pending;
        };

        let TaskFuture::Future(fut) = &mut *future else {
            // this future is already completed
            return Poll::Ready(());
        };

        let waker = self.clone().waker();
        let mut cx = Context::from_waker(&waker);

        if fut.as_mut().poll(&mut cx).is_ready() {
            *future = TaskFuture::Done;
            self.0.join.wake();

            Poll::Ready(())
        } else {
            Poll::Pending
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
    join: AtomicWaker,
}

impl TaskInner {
    pub fn new(fut: Pin<Box<dyn Future<Output = ()> + Send>>) -> Self {
        Self {
            future: Mutex::new(TaskFuture::Future(fut)),
            join: AtomicWaker::new(),
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
    Done,
}

//

pub struct JoinHandle<T: Send> {
    task: Task,
    result: Arc<Mutex<Option<T>>>,
}

impl<T: Send> JoinHandle<T> {
    pub fn spawn<F>(fut: F) -> Self
    where
        F: IntoFuture<Output = T>,
        F::IntoFuture: Send + 'static,
        F::Output: Send + 'static,
    {
        let result_tx = Arc::new(Mutex::new(None));
        let result_rx = result_tx.clone();

        let fut = fut.into_future();
        let task = async move {
            *result_tx.try_lock().unwrap() = Some(fut.await);
        }
        .into_task();
        executor::push_task(task.clone());

        JoinHandle {
            task: task.clone(),
            result: result_rx,
        }
    }

    fn take_result(&self) -> T {
        self.result.try_lock().unwrap().take().unwrap()
    }
}

impl<T: Send> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.task.clone().poll().is_ready() {
            return Poll::Ready(self.take_result());
        }

        // if matches!(&*future, TaskFuture::Done) {
        //     return Poll::Ready(());
        // }

        self.task.0.join.register(cx.waker());

        // if matches!(&*future, TaskFuture::Done) {
        if self.task.clone().poll().is_ready() {
            self.task.0.join.take();
            Poll::Ready(self.take_result())
        } else {
            Poll::Pending
        }
    }
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
