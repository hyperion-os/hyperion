use crate::debug;

use super::task::{IntoTask, Task, TaskId};
use alloc::{boxed::Box, sync::Arc};
use core::task::{Context, Poll, RawWaker, RawWakerVTable};
use crossbeam_queue::{ArrayQueue, SegQueue};
use spin::{Mutex, MutexGuard};

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

pub struct Executor {
    free_task_ids: Arc<ArrayQueue<TaskId>>,
    task_ids: Arc<ArrayQueue<TaskId>>,
    tasks: Arc<[Mutex<Option<Task>>]>,
}

impl Executor {
    pub fn new() -> Self {
        let free_task_ids = Arc::new(ArrayQueue::new(256));
        for i in 0..=255 {
            _ = free_task_ids.push(TaskId(i));
        }

        Self {
            free_task_ids,
            task_ids: Arc::new(ArrayQueue::new(256)),
            tasks: (0..=255).map(|_| Mutex::new(None)).collect::<Arc<_>>(),
        }
    }

    pub fn next_task_id(&self) -> TaskId {
        self.free_task_ids.pop().expect("task queue full")
    }

    pub fn free_task_id(&self, task: TaskId) {
        self.free_task_ids.push(task).expect("task queue full");
    }

    pub fn add_task(&self, task: impl IntoTask) {
        let task = task.into_task(self);
        let id = task.id;
        // this lock should never block
        *self.tasks[id.0 as usize].lock() = Some(task);
        self.task_ids.push(id).unwrap();
    }

    pub fn take_task(&self) -> Option<Task> {
        let id = self.task_ids.pop()?;
        // this lock should never block
        self.tasks[id.0 as usize].lock().take()
    }

    pub fn run(&self) {
        while let Some(mut task) = self.take_task() {
            let waker = Waker::new();
            let waker = waker.waker();
            let mut ctx = Context::from_waker(&waker);

            match task.poll(&mut ctx) {
                Poll::Ready(()) => self.free_task_id(task.id),
                Poll::Pending => self.add_task(task),
            }
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}
