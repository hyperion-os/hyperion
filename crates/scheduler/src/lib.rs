#![no_std]

extern crate alloc;

//

use core::time::Duration;

use alloc::boxed::Box;
use crossbeam_queue::SegQueue;
use spin::Once;

use self::task::Task;

//

pub mod task;

//

pub fn run_forever() -> ! {
    loop {
        run_all();
    }
}

pub fn run_all() {
    while run_once().is_some() {}
}

pub fn run_once() -> Option<()> {
    TASKS.pop()?.poll();
    Some(())
}

pub fn spawn(fut: impl Into<Task>) {
    TASKS.push(fut.into())
}

//

pub static TASKS: SegQueue<Task> = SegQueue::new();
pub static EXECUTOR: Once<&'static dyn ExecutorBackend> = Once::new();

//

pub trait ExecutorBackend: Sync {
    /// call `f` after `delay`
    fn run_after(&self, delay: Duration, f: Box<dyn FnOnce()>);
}

//

pub struct DefaultExecutor;

impl ExecutorBackend for DefaultExecutor {
    fn run_after(&self, _: Duration, _: Box<dyn FnOnce()>) {
        // default executor doesn't understand time
    }
}
