#![no_std]

//

extern crate alloc;

use alloc::{string::String, sync::Arc};
use core::num::ParseIntError;

use futures_util::StreamExt;
use hyperion_color::Color;
use hyperion_framebuffer::framebuffer::Framebuffer;
use hyperion_futures::{keyboard::KeyboardEvents, timer::ticks};
use hyperion_kernel_impl::VFS_ROOT;
use hyperion_random::Rng;
use hyperion_scheduler::lock::Mutex;
use hyperion_vfs::{error::IoError, path::PathBuf, ramdisk};
use snafu::Snafu;
use time::Duration;

use self::{shell::Shell, term::Term};

//

pub mod shell;
pub mod snake;
pub mod term;

//

pub async fn kshell() {
    hyperion_futures::executor::spawn(spinner());

    // TODO: initrd
    let bin = include_bytes!(env!("CARGO_BIN_FILE_SAMPLE_ELF"));
    VFS_ROOT.install_dev("/bin/run", ramdisk::File::new(bin.into()));
    let bin = include_bytes!(env!("CARGO_BIN_FILE_COREUTILS"));
    let bin = Arc::new(Mutex::new(ramdisk::File::new(bin.into())));

    VFS_ROOT.install_dev_ref("/bin/coreutils", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/cat", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/ls", bin.clone());
    VFS_ROOT.install_dev_ref("/bin/touch", bin);

    let term = Term::new();
    let mut shell = Shell::new(term);

    shell.init();
    while let Some(ev) = KeyboardEvents.next().await {
        if shell.input(ev).await.is_none() {
            break;
        }
    }

    let mut term = shell.into_inner();
    term.clear();
    term.flush();
}

pub async fn spinner() {
    let mut ticks = ticks(Duration::milliseconds(50));
    let mut rng = hyperion_random::next_fast_rng();

    while ticks.next().await.is_some() {
        let Some(fbo) = Framebuffer::get() else {
            continue;
        };
        let mut fbo = fbo.lock();

        let (r, g, b) = rng.gen();
        let x = fbo.width - 20;
        let y = fbo.height - 20;
        fbo.fill(x, y, 10, 10, Color::new(r, g, b));
    }
}

//

const CHAR_SIZE: (u8, u8) = (8, 16);
// const WIDE_CHAR_SIZE: (u8, u8) = (16, 16);

//

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("VFS error: {source}"))]
    IoError {
        source: IoError,
        resource: Option<PathBuf>,
    },

    #[snafu(display("VFS error: Nameless file"))]
    NamelessFile,

    #[snafu(display("Parse error: {source}"))]
    Parse { source: ParseIntError },

    #[snafu(display("{msg}"))]
    Other { msg: String },

    #[snafu(display("insecure PRNG"))]
    InsecurePrng,
}

pub type Result<T> = core::result::Result<T, Error>;
