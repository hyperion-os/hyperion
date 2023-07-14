use alloc::{boxed::Box, sync::Arc};
use core::{
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::Context,
};

use futures_util::{
    task::{waker, ArcWake},
    Future,
};
use hyperion_log::warn;
use spin::Mutex;

use crate::executor;

//

pub struct Task {
    complete: AtomicBool,
    ctx: Mutex<TaskContext>,
}

pub enum TaskContext {
    /// A kernel task
    Future {
        inner: Pin<Box<dyn Future<Output = ()> + Send>>,
    },

    None,
}

//

impl Task {
    pub fn from_future(fut: impl Future<Output = ()> + Send + 'static) -> Self {
        Self {
            complete: AtomicBool::new(false),
            ctx: Mutex::new(TaskContext::Future {
                inner: Box::pin(fut),
            }),
        }
    }

    pub fn poll(self: Arc<Self>) {
        if self.complete.load(Ordering::Acquire) {
            warn!("already complete");
            return;
        }

        let Some(mut ctx) = self.ctx.try_lock() else {
            // another CPU is already working on this task
            return;
        };

        match &mut *ctx {
            TaskContext::Future { inner } => {
                let waker = waker(self.clone());
                let mut ctx = Context::from_waker(&waker);

                if inner.as_mut().poll(&mut ctx).is_ready() {
                    self.complete.store(true, Ordering::Release);
                }
            }
            TaskContext::None => {
                self.complete.store(true, Ordering::Release);
            }
        }
    }

    pub fn schedule(self: &Arc<Self>) {
        executor::push_task(self.clone())
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.schedule();
    }
}
