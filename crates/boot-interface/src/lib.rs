#![no_std]

//

pub use framebuffer::*;
pub use loader::*;
pub use map::*;
pub use smp::*;

//

mod framebuffer;
mod loader;
mod map;
mod smp;

//

extern "Rust" {
    pub fn kernel_main() -> !;
}
