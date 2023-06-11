use std::{
    env::{self, var},
    error::Error,
    fs,
    path::PathBuf,
};

//

fn main() -> Result<(), Box<dyn Error>> {
    let kernel = var("CARGO_PKG_NAME")?;
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
    //let arch = var("CARGO_CFG_TARGET_ARCH")?;
    //println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");

    let mut bootloader: Option<&'static str> = None;
    let mut set = |s| {
        if let Some(already) = bootloader {
            println!("cargo:warning=Bootloaders {s} and {already} are mutually exclusive");
            panic!();
        } else {
            bootloader = Some(s);
        }
    };
    #[cfg(feature = "limine")]
    set("limine");
    #[cfg(feature = "bootboot")]
    set("bootboot");
    #[cfg(feature = "multiboot1")]
    set("multiboot1");
    #[cfg(feature = "multiboot2")]
    set("multiboot2");

    if let Some(bootloader) = bootloader {
        let script = format!("crates/boot-{bootloader}/src/link.ld");
        println!("cargo:rustc-link-arg-bin={kernel}=--script={script}");
        println!("cargo:rerun-if-changed={script}");
    } else {
        println!("cargo:warning=No bootloaders given");
        panic!();
    };

    Ok(())
}
