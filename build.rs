use std::{
    env::var,
    error::Error,
    fs::{self, File, OpenOptions},
    io::Read,
    io::Write,
    path::PathBuf,
    process::Command,
};

//

fn main() -> Result<(), Box<dyn Error>> {
    let kernel = var("CARGO_PKG_NAME")?;
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
    let arch = var("CARGO_CFG_TARGET_ARCH")?;
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");

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
        let script = format!("src/arch/{arch}/{bootloader}/link.ld");
        println!("cargo:rustc-link-arg-bin={kernel}=--script={script}");
        println!("cargo:rerun-if-changed={script}");
    } else {
        println!("cargo:warning=No bootloaders given");
        panic!();
    };

    let unifont_path = "target/hyperion/unifont.bmp";
    let read_unifont = || {
        let mut file = OpenOptions::new()
            .read(true)
            .create(false)
            .write(false)
            .open(unifont_path)?;

        let mut buf = Vec::new();
        file.read_to_end(buf)?;
        Ok::<_, Box<dyn Error>>(buf)
    };

    let unifont = if let Ok(file) = read_unifont() {
        file
    } else {
        Command::new("wget")
            .arg("http://unifoundry.com/pub/unifont/unifont-15.0.01/unifont-15.0.01.bmp")
            .args(["-O", unifont_path])
            .spawn()
            .unwrap();

        read_unifont().unwrap()
    };

    Ok(())
}
