pub use decode::{layouts, set_layout};

//

#[cfg(feature = "input-buffer")]
mod decode;

#[cfg(feature = "input-buffer")]
pub mod buffer;
pub mod event;
