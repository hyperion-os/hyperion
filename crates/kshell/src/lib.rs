#![no_std]

//

extern crate alloc;

use alloc::sync::Arc;

use futures_util::StreamExt;
use hyperion_kernel_impl::VFS_ROOT;
use hyperion_log::*;
use hyperion_scheduler::lock::Mutex;
use hyperion_vfs::ramdisk::StaticRoFile;

use crate::{shell::Shell, term::Term};

//

pub mod cmd;
pub mod shell;
pub mod term;

//

macro_rules! load_elf {
    ($bin:literal) => {
        load_elf_from!(env!(concat!("CARGO_BIN_FILE_", $bin)))
    };
}

macro_rules! load_elf_from {
    ($($t:tt)*) => {{
        const FILE: &[u8] = include_bytes!($($t)*);
        trace!("ELF from {}", $($t)*);
        FILE
    }};
}

//

include!(concat!(env!("OUT_DIR"), "/asset.rs"));

//

pub async fn kshell() {
    // hyperion_futures::executor::spawn(spinner());

    // TODO: initrd

    for asset in ASSETS {
        let (path, bytes): (&str, &[u8]) = *asset;
        VFS_ROOT.install_dev(path, StaticRoFile::new(bytes));
    }

    VFS_ROOT.install_dev("/bin/run", StaticRoFile::new(load_elf!("SAMPLE_ELF")));
    VFS_ROOT.install_dev("/bin/fbtest", StaticRoFile::new(load_elf!("FBTEST")));

    // everything is the same file
    let bin = Arc::new(Mutex::new(StaticRoFile::new(load_elf!("COREUTILS"))));
    VFS_ROOT.install_dev_ref("/bin/cat", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/cp", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/date", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/echo", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/hello", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/ls", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/mem", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/mkdir", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/nproc", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/ps", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/random", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/sleep", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/tail", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/top", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/touch", bin.clone());

    VFS_ROOT.install_dev_ref("/bin/coreutils", bin);

    let term = Term::new();
    let mut shell = Shell::new(term);

    shell.init();

    while shell.run().await.is_some() {}

    let mut term = shell.into_inner();
    term.clear();
    term.flush();
}

// async fn spinner() {
//     let mut ticks = ticks(Duration::milliseconds(50));
//     let mut rng = hyperion_random::next_fast_rng();

//     while ticks.next().await.is_some() {
//         let Some(fbo) = Framebuffer::get() else {
//             continue;
//         };
//         let mut fbo = fbo.lock();

//         let (r, g, b) = rng.gen();
//         let x = fbo.width - 20;
//         let y = fbo.height - 20;
//         fbo.fill(x, y, 10, 10, Color::new(r, g, b));
//     }
// }

//

const CHAR_SIZE: (u8, u8) = (8, 16);
// const WIDE_CHAR_SIZE: (u8, u8) = (16, 16);
