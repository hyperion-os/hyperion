use futures_util::StreamExt;
use snafu::Snafu;

use crate::{
    driver::video::framebuffer::Framebuffer,
    log,
    vfs::{path::PathBuf, IoError},
};

use self::{shell::Shell, term::Term};

use super::keyboard::KeyboardEvents;

//

pub mod shell;
pub mod term;

//

pub async fn kshell() {
    log::disable_fbo();
    let Some(mut vbo) = Framebuffer::get() else {
        // TODO: serial only
        panic!("cannot run kshell without a framebuffer");
    };

    let term = Term::new(&mut vbo);
    let mut shell = Shell::new(term);
    let mut ev = KeyboardEvents::new();

    shell.init();
    while let Some(ev) = ev.next().await {
        shell.input(ev)
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
}

pub type Result<T> = core::result::Result<T, Error>;
