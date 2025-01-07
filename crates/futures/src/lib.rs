#![no_std]

//

extern crate alloc;

//

pub mod keyboard;
pub mod lazy;
pub mod lock;
pub mod map;
pub mod mouse;
pub mod mpmc;
pub mod timer;

mod block;
mod executor;
mod task;

//

pub use block::block_on;
pub use executor::{run_tasks, spawn};
