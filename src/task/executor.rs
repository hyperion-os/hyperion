use super::task::{Task, TaskId};
use alloc::{boxed::Box, sync::Arc};
use core::task::{Context, Poll, RawWaker, RawWakerVTable};
use crossbeam_queue::{ArrayQueue, SegQueue};
use spin::Mutex;

//

#[derive(Clone, Copy, Default)]
pub struct Waker {}

impl Waker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn raw(&self) -> RawWaker {
        fn noop(_: *const ()) {}

        let vtable = &RawWakerVTable::new(Self::clone, noop, noop, Self::drop);
        RawWaker::new(self as *const Self as _, vtable)
    }

    fn read_self_ptr(waker: *const ()) -> &'static Self {
        unsafe { &*(waker as *const Self) }
    }

    fn clone(waker: *const ()) -> RawWaker {
        let waker = Self::read_self_ptr(waker);
        Box::leak(Box::new(*waker)).raw()
    }

    fn drop(waker: *const ()) {
        unsafe {
            _ = Box::from_raw(waker as *mut Self);
        }
    }

    pub fn waker(&self) -> core::task::Waker {
        unsafe { core::task::Waker::from_raw(self.raw()) }
    }
}

#[derive(Default)]
pub struct Executor {
    /* free_task_ids: ArrayQueue<TaskId>,
    task_ids: ArrayQueue<TaskId>,
    tasks: [Mutex<Task>; 256], */
    tasks: SegQueue<Task>,
}

impl Executor {
    pub const fn new() -> Self {
        Self {
            tasks: SegQueue::new(),
        }
    }

    pub fn add_task(&self, task: impl Into<Task>) {
        self.tasks.push(task.into())
    }

    pub fn take_task(&self) -> Option<Task> {
        self.tasks.pop()
    }

    pub fn run(&self) {
        while let Some(mut task) = self.take_task() {
            let waker = Waker::new();
            let waker = waker.waker();
            let mut ctx = Context::from_waker(&waker);
            match task.poll(&mut ctx) {
                Poll::Ready(()) => break,
                Poll::Pending => self.add_task(task),
            }
        }
    }
}
