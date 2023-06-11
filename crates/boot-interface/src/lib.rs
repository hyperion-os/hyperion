#![no_std]

//

pub use framebuffer::*;
pub use map::*;
pub use smp::*;

//

mod framebuffer;
mod map;
mod smp;

//

extern "Rust" {
    pub fn kernel_main() -> !;
}
