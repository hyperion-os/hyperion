//! Kernel built in shell, the default init program

use crate::{
    driver::video::{color::Color, framebuffer::Framebuffer},
    log,
    mem::pmm::PageFrameAllocator,
    scheduler::keyboard::KeyboardEvents,
    util::fmt::NumberPostfix,
    vfs::{self, path::Path, IoError, Node},
    KERNEL_BUILD_REV, KERNEL_BUILD_TIME, KERNEL_NAME, KERNEL_SPLASH, KERNEL_VERSION,
};
use alloc::{boxed::Box, string::String};
use chrono::{TimeZone, Utc};
use core::fmt::{self, Debug, Write};
use futures_util::StreamExt;
use snafu::{ResultExt, Snafu};

//

pub async fn kshell() {
    log::disable_fbo();
    let Some(mut vbo) = Framebuffer::get() else {
        // TODO: serial only
        panic!("cannot run kshell without a framebuffer");
    };

    let mut term = Term::new(&mut vbo);
    _ = splash_cmd(&mut term, None);
    term.prompt();
    term.flush();

    let mut ev = KeyboardEvents::new();
    let mut cmdbuf = String::new();

    while let Some(ev) = ev.next().await {
        if ev == '\n' {
            _ = writeln!(term);
            if let Err(err) = run_line(&mut term, &cmdbuf) {
                _ = writeln!(term, "{err}");
            };
            cmdbuf.clear();
            term.prompt();
        } else if ev == '\u{8}' {
            if cmdbuf.pop().is_some() {
                term.cursor_prev();
                let cursor = term.cursor;
                term.write_byte(b' ');
                term.cursor = cursor;
            }
        } else {
            _ = write!(term, "{ev}");
            cmdbuf.push(ev);
        }

        term.flush();
    }
}

//

const CHAR_SIZE: (u8, u8) = (8, 16);
// const WIDE_CHAR_SIZE: (u8, u8) = (16, 16);

//

struct Term<'fbo> {
    cursor: (usize, usize),
    size: (usize, usize),
    buf: Box<[u8]>,
    old_buf: Box<[u8]>,
    vbo: &'fbo mut Framebuffer,
}

#[derive(Debug, Snafu)]
enum Error<'a> {
    #[snafu(display("VFS error: {source}"))]
    IoError {
        source: IoError,
        resource: Option<&'a Path>,
    },

    #[snafu(display("VFS error: Nameless file"))]
    NamelessFile,
}

type Result<'a, T> = core::result::Result<T, Error<'a>>;

//

impl<'fbo> Term<'fbo> {
    fn new(vbo: &'fbo mut Framebuffer) -> Self {
        let vbo_info = vbo.info();

        let cursor = (0, 0);

        let size = (
            vbo_info.width / CHAR_SIZE.0 as usize,
            vbo_info.height / CHAR_SIZE.1 as usize,
        );

        let buf = (0..size.0 * size.1).map(|_| b' ').collect();
        let old_buf = (0..size.0 * size.1).map(|_| b'=').collect();

        Self {
            cursor,
            size,
            buf,
            old_buf,
            vbo,
        }
    }

    fn prompt(&mut self) {
        self.write_bytes(b"\n[shell] > ");
    }

    fn flush(&mut self) {
        // let positions = (0..self.size.1).flat_map(|y| (0..self.size.0).map(move |x| (x, y)));

        // let mut updates = 0u32;
        for ((idx, ch), _) in self
            .buf
            .iter()
            .enumerate()
            .zip(self.old_buf.iter())
            .filter(|((_, b1), b0)| **b1 != **b0)
        {
            let x = (idx % self.size.0) * CHAR_SIZE.0 as usize;
            let y = (idx / self.size.0) * CHAR_SIZE.1 as usize;

            // updates += 1;
            self.vbo.ascii_char(x, y, *ch, Color::WHITE, Color::BLACK);
        }
        // debug!("updates: {updates}");
        self.old_buf.copy_from_slice(&self.buf);
    }

    fn cursor_next(&mut self) {
        self.cursor.0 += 1;

        if self.cursor.0 >= self.size.0 {
            self.cursor.0 = 0;
            self.cursor.1 += 1;
        }
    }

    fn cursor_prev(&mut self) {
        if self.cursor.0 == 0 {
            if self.cursor.1 == 0 {
                return;
            }
            self.cursor.0 = self.size.0 - 1;
            self.cursor.1 -= 1;
        }

        self.cursor.0 -= 1;
    }

    fn write_bytes(&mut self, b: &[u8]) {
        for b in b {
            self.write_byte(*b);
        }
    }

    fn write_byte(&mut self, b: u8) {
        if self.cursor.0 >= self.size.0 {
            self.cursor.0 = 0;
            self.cursor.1 += 1;
        }
        if self.cursor.1 >= self.size.1 {
            let len = self.buf.len();
            self.cursor.1 = self.size.1 - 1;
            self.buf.copy_within(self.size.0.., 0);
            self.buf[len - self.size.0..].fill(b' ');
        }

        // crate::debug!("{b}");
        match b {
            b'\n' => {
                self.cursor.0 = 0;
                self.cursor.1 += 1;
            }
            other => {
                self.buf[self.cursor.0 + self.cursor.1 * self.size.0] = other;
                self.cursor.0 += 1;
            }
        }
    }
}

impl<'fbo> fmt::Write for Term<'fbo> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());
        Ok(())
    }
}

//

fn run_line<'a>(term: &mut Term, line: &'a str) -> Result<'a, ()> {
    let (cmd, args) = line
        .split_once(' ')
        .map(|(cmd, args)| (cmd, Some(args)))
        .unwrap_or((line, None));

    match cmd {
        "splash" => splash_cmd(term, args)?,
        "ls" => ls_cmd(term, args)?,
        "cat" => cat_cmd(term, args)?,
        "date" => date_cmd(term, args)?,
        "mem" => mem_cmd(term, args)?,
        "clear" => {
            term.cursor = (0, 0);
            term.buf.fill(b' ');
        }
        "" => term.write_byte(b'\n'),
        other => {
            _ = writeln!(term, "unknown command {other}");
        }
    }

    Ok(())
}

fn splash_cmd<'a>(term: &mut Term, _: Option<&'a str>) -> Result<'a, ()> {
    _ = writeln!(term, "{KERNEL_SPLASH}");
    _ = writeln!(term, "Welcome to {KERNEL_NAME} - {KERNEL_VERSION} (built {KERNEL_BUILD_TIME} build [{KERNEL_BUILD_REV}])");
    Ok(())
}

fn ls_cmd<'a>(term: &mut Term, args: Option<&'a str>) -> Result<'a, ()> {
    let resource = Path::from_str(args.unwrap_or("/"));
    let dir = vfs::get_node(resource, false).context(IoSnafu { resource })?;

    match dir {
        Node::File(_) => {
            if let Some(file_name) = resource.file_name() {
                _ = writeln!(term, "{file_name}");
            } else {
                return Err(Error::NamelessFile);
            }
        }
        Node::Directory(dir) => {
            let mut dir = dir.lock();
            for entry in dir.nodes().context(IoSnafu { resource })? {
                _ = writeln!(term, "{entry}");
            }
        }
    }

    Ok(())
}

fn cat_cmd<'a>(term: &mut Term, args: Option<&'a str>) -> Result<'a, ()> {
    let resource = Path::from_str(args.unwrap_or("/"));
    let file = vfs::get_file(resource, false, false).context(IoSnafu { resource })?;
    let mut file = file.lock();

    let mut at = 0usize;
    let mut buf = [0u8; 16];
    loop {
        let read = file.read(at, &mut buf).context(IoSnafu { resource })?;
        if read == 0 {
            break;
        }
        at += read;

        for byte in buf {
            term.write_byte(byte);
        }
        term.write_byte(b'\n');
    }

    Ok(())
}

fn date_cmd<'a>(term: &mut Term, _: Option<&'a str>) -> Result<'a, ()> {
    let resource = Path::from_str("/dev/rtc");
    let file = vfs::get_file(resource, false, false).context(IoSnafu { resource })?;
    let mut file = file.lock();

    let mut timestamp = [0u8; 8];
    file.read_exact(0, &mut timestamp)
        .context(IoSnafu { resource })?;

    let date = Utc.timestamp_nanos(i64::from_le_bytes(timestamp));

    _ = writeln!(term, "{date:?}");

    Ok(())
}

fn mem_cmd<'a>(term: &mut Term, _: Option<&'a str>) -> Result<'a, ()> {
    let pfa = PageFrameAllocator::get();
    let used = pfa.used_mem();
    let usable = pfa.usable_mem();
    _ = writeln!(
        term,
        "Mem:\n - total: {}B\n - used: {}B ({:3.1}%)",
        usable.postfix_binary(),
        used.postfix_binary(),
        used as f64 / usable as f64 * 100.0
    );

    Ok(())
}
