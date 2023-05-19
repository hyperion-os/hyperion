use futures_util::{stream::select, StreamExt};
use snafu::Snafu;

use crate::{
    driver::video::framebuffer::Framebuffer,
    log,
    vfs::{path::PathBuf, IoError},
};

use self::{shell::Shell, term::Term};

use super::{keyboard::KeyboardEvents, tick::Ticks};

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
    let ev = KeyboardEvents::new();
    let tick = Ticks::new();
    let mut stream = select(ev.map(Some), tick.map(|_| None));

    shell.init();
    while let Some(ev) = stream.next().await {
        if let Some(char) = ev {
            shell.input(char);
        } else {
            shell.tick();
        }
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
