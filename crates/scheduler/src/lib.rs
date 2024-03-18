#![no_std]

extern crate alloc;

//

use self::task::RunnableTask;

//

pub mod proc;
pub mod task;

//

/// terminate the active task and enter the async scheduler
pub fn init() -> ! {
    RunnableTask::next().enter();
}
