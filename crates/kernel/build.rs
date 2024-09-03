use std::{
    env::{self, var},
    error::Error,
    process::Command,
};

//

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=./build.rs");
    println!("cargo:rerun-if-changed=../../Cargo.lock");

    let kernel = var("CARGO_PKG_NAME")?;
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME .");
    //let arch = var("CARGO_CFG_TARGET_ARCH")?;
    //println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");

    println!("cargo:rustc-link-arg=-no-pie");
    //println!("cargo:rust-link-arg=-no-pie");

    let mut bootloader: Option<&'static str> = None;
    let mut set = |s| {
        if let Some(already) = bootloader {
            println!("cargo:warning=Bootloaders {s} and {already} are mutually exclusive");
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
        // println!("cargo:rustc-link-arg-bin={kernel}=-T");
        // println!("cargo:rustc-link-arg-bin={kernel}={script}");
        println!("cargo:rerun-if-changed=../../{script}");
    } else {
        println!("cargo:warning=No bootloaders given");
        panic!();
    };

    Ok(())
}
