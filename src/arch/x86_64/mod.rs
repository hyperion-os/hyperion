#[cfg(feature = "multiboot1")]
#[path = "multiboot1/mod.rs"]
pub mod boot;
#[cfg(feature = "multiboot2")]
#[path = "multiboot2/mod.rs"]
pub mod boot;
#[cfg(feature = "bootboot")]
#[path = "bootboot/mod.rs"]
pub mod boot;
#[cfg(feature = "limine")]
#[path = "limine/mod.rs"]
pub mod boot;
pub use boot::*;

pub mod gdt;
pub mod idt;
