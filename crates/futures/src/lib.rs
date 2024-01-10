#![no_std]
#![feature(const_mut_refs, const_unsafecell_get_mut)]

//

extern crate alloc;

//

pub mod keyboard;
pub mod lock;
pub mod mouse;
pub mod mpmc;
pub mod timer;

mod block;
mod executor;
mod task;

//

pub use block::block_on;
pub use executor::{run_tasks, spawn};
