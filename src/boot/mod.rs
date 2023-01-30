use spin::Once;

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

//

pub use boot::*;

//

/// Name of the detected bootloader
pub static BOOT_NAME: Once<&'static str> = Once::new();
