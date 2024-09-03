#![no_std]

//

extern crate alloc;

//

mod mpmc;

pub mod keyboard;
pub mod mouse;
pub mod timer;

//

pub use mpmc::Recv;
