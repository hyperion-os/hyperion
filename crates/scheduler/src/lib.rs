#![no_std]

//

extern crate alloc;

pub mod executor;
pub mod keyboard;
pub mod process;
pub mod task;
pub mod timer;

//

use alloc::boxed::Box;
use core::{
    cell::UnsafeCell,
    mem::swap,
    sync::atomic::{AtomicUsize, Ordering},
};

use crossbeam_queue::SegQueue;
use hyperion_arch::{context::Context, tls};
use hyperion_mem::pmm::{PageFrame, PageFrameAllocator};
use hyperion_scheduler_task::{AnyTask, CleanupTask, Task};

//

pub struct TaskImpl {
    // context is used 'unsafely' only in the switch
    context: UnsafeCell<Context>,
    stack: Option<PageFrame>,
    job: Option<Box<dyn FnOnce() + Send + 'static>>,
    pid: usize,
}

impl TaskImpl {
    pub fn new(f: impl FnOnce() + Send + 'static) -> Self {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(0);
        let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);

        let mut stack = PageFrameAllocator::get().alloc(10);
        let context = UnsafeCell::new(Context::new(stack.as_mut_slice(), thread_entry));
        let stack = Some(stack);
        let job = Some(Box::new(f) as _);

        Self {
            context,
            stack,
            job,
            pid,
        }
    }

    pub fn debug(&mut self) {
        hyperion_log::debug!(
            "TASK DEBUG: context: {:0x}, job: {:?}, pid: {}",
            unsafe { (*self.context.get()).rsp },
            self.job.as_ref().map(|_| ()),
            self.pid
        )
    }
}

impl AnyTask for TaskImpl {
    fn context(&mut self) -> *mut () {
        self.context.get() as _
    }

    fn take_job(&mut self) -> Option<Box<dyn FnOnce() + Send + 'static>> {
        self.job.take()
    }

    fn pid(&self) -> usize {
        self.pid
    }
}

impl Drop for TaskImpl {
    fn drop(&mut self) {
        if let Some(stack) = self.stack.take() {
            PageFrameAllocator::get().free(stack)
        }
    }
}

pub static READY: SegQueue<Task> = SegQueue::new();

/// reset this processors scheduling
pub fn reset() -> ! {
    let boot: Task = Box::new(TaskImpl::new(|| {}));
    *tls::get().active.lock() = Some(boot);
    stop();
}

/// switch to another thread
pub fn yield_now() {
    let Some(mut current) = swap_current(None) else {
        unreachable!("cannot yield from a task that doesn't exist")
    };

    // push the current thread back to the ready queue AFTER switching
    // current.debug();
    let context = current.context() as *mut Context;
    tls::get().after_switch.push(CleanupTask::Ready(current));

    // SAFETY: `current` is stored in the queue until the switch
    // and the boxed field `context` makes sure the context pointer doesn't move
    unsafe {
        block(context);
    }
}

/// destroy the current thread
/// and switch to another thread
pub fn stop() -> ! {
    // hyperion_log::debug!("stop");
    let Some(mut current) = swap_current(None) else {
        unreachable!("cannot stop a task that doesn't exist")
    };

    // push the current thread to the drop queue AFTER switching
    // current.debug();
    let context = current.context() as *mut Context;
    tls::get().after_switch.push(CleanupTask::Drop(current));

    // SAFETY: `current` is stored in the queue until the switch
    // and the boxed field `context` makes sure the context pointer doesn't move
    unsafe {
        block(context);
    }

    unreachable!("a destroyed thread cannot continue executing");
}

pub fn spawn(f: impl FnOnce() + Send + 'static) {
    schedule(Box::new(TaskImpl::new(f)))
}

/// schedule
pub fn schedule(new: Task) {
    READY.push(new);
}

pub fn swap_current(mut new: Option<Task>) -> Option<Task> {
    swap(&mut new, &mut tls::get().active.lock());
    new
}

/// # Safety
///
/// `current` must be correct and point to a valid exclusive [`Context`]
pub unsafe fn block(current: *mut Context) {
    let mut next = next_task();

    // next.debug();
    let context = next.context() as *mut Context;
    tls::get().after_switch.push(CleanupTask::Next(next));

    // SAFETY: `next` is stored in the queue until the switch
    // and the boxed field `context` makes sure the context pointer doesn't move
    unsafe {
        hyperion_arch::context::switch(current, context);
    }

    cleanup();
}

pub fn next_task() -> Task {
    // loop {
    for _ in 0..1000 {
        if let Some(next) = READY.pop() {
            return next;
        }

        // hyperion_log::debug!("no jobs");

        // TODO: halt until the next task arrives
    }

    // give up and run a none task
    Box::new(TaskImpl::new(|| {}))
}

pub fn cleanup() {
    loop {
        match tls::get().after_switch.pop() {
            Some(CleanupTask::Ready(ready)) => {
                schedule(ready);
            }
            Some(CleanupTask::Next(next)) => {
                swap_current(Some(next));
            }
            Some(CleanupTask::Drop(_drop)) => {}
            None => break,
        };
    }
}

extern "sysv64" fn thread_entry() -> ! {
    cleanup();
    {
        let Some(mut current) = swap_current(None) else {
            unreachable!("cannot run a task that doesn't exist")
        };
        let Some(job) = current.take_job() else {
            unreachable!("cannot run a task that already ran")
        };
        swap_current(Some(current));
        job();
    }
    stop();
}
