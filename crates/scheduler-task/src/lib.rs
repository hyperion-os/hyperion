#![no_std]

//

extern crate alloc;

use alloc::boxed::Box;

//

pub type Task = Box<dyn AnyTask + Send>;

pub trait AnyTask {
    /// the actual type depends on the arch
    fn context(&mut self) -> *mut ();

    fn take_job(&mut self) -> Option<Box<dyn FnOnce() + Send + 'static>>;

    fn pid(&self) -> usize;
}

pub enum CleanupTask {
    Next(Task),
    Drop(Task),
    Ready(Task),
}
