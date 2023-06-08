use alloc::string::String;
use core::num::ParseIntError;

use futures_util::StreamExt;
use hyperion_vfs::{path::PathBuf, IoError};
use snafu::Snafu;

use self::{shell::Shell, term::Term};
use super::keyboard::KeyboardEvents;
use crate::driver::video::framebuffer::Framebuffer;

//

pub mod shell;
pub mod term;

//

pub async fn kshell() {
    let term = Term::new();
    let mut shell = Shell::new(term);
    let mut stream = KeyboardEvents::new();

    shell.init();
    while let Some(ev) = stream.next().await {
        shell.input(ev).await;
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
}

pub type Result<T> = core::result::Result<T, Error>;
