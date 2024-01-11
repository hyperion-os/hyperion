#![no_std]
#![feature(const_binary_heap_constructor)]

//

extern crate alloc;

//

mod mpmc;

pub mod keyboard;
pub mod mouse;
pub mod timer;

//

pub use mpmc::Recv;
