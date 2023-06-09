pub use boot::*;

//

#[cfg(feature = "multiboot1")]
#[path = "multiboot1/mod.rs"]
#[allow(clippy::module_inception)]
mod boot;
#[cfg(feature = "multiboot2")]
#[path = "multiboot2/mod.rs"]
#[allow(clippy::module_inception)]
mod boot;
#[cfg(feature = "bootboot")]
#[path = "bootboot/mod.rs"]
#[allow(clippy::module_inception)]
mod boot;
#[cfg(feature = "limine")]
#[path = "limine/mod.rs"]
#[allow(clippy::module_inception)]
mod boot;

pub mod args;
