use std::{env::args, process::Command};

//

fn main() {
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    let uefi = !args().find(|s| s == "--bios").is_some();

    let mut cmd = Command::new("qemu-system-x86_64");
    if uefi {
        cmd.arg("-bios")
            .arg(ovmf_prebuilt::ovmf_pure_efi())
            .arg("-drive")
            .arg(format!("format=raw,file={uefi_path}"));
    } else {
        cmd.arg("-drive")
            .arg(format!("format=raw,file={bios_path}"));
    }
    cmd.spawn().unwrap().wait().unwrap();
}
