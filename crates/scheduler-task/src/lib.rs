#![no_std]

//

extern crate alloc;

use alloc::boxed::Box;
use core::any::Any;

//

pub type Task = Box<dyn AnyTask + Send>;

pub trait AnyTask {
    fn as_any(&mut self) -> &mut dyn Any;

    fn take_job(&mut self) -> Option<Box<dyn FnOnce() + Send + 'static>>;

    fn pid(&self) -> usize;
}

pub enum CleanupTask {
    Next(Task),
    Drop(Task),
    Ready(Task),
}
