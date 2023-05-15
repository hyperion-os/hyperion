use alloc::boxed::Box;
use core::{
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
};
use futures_util::Future;

//

pub struct Task {
    id: TaskId,
    fut: Pin<Box<dyn Future<Output = ()>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TaskId(u64);

//

impl Task {
    pub fn poll(&mut self, ctx: &mut Context) -> Poll<()> {
        self.fut.as_mut().poll(ctx)
    }
}

impl<F: Future<Output = ()> + 'static> From<F> for Task {
    fn from(value: F) -> Self {
        Self {
            id: TaskId::next(),
            fut: Box::pin(value),
        }
    }
}

impl TaskId {
    pub fn next() -> Self {
        static IDS: AtomicU64 = AtomicU64::new(0);
        Self(IDS.fetch_add(1, Ordering::Relaxed))
    }
}
