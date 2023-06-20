use alloc::{string::String, sync::Arc};
use core::fmt::Write;

use hyperion_color::Color;
use hyperion_keyboard::{event::KeyboardEvent, layouts, set_layout};
use hyperion_mem::pmm::PageFrameAllocator;
use hyperion_num_postfix::NumberPostfix;
use hyperion_random::Rng;
use hyperion_scheduler::timer::sleep;
use hyperion_vfs::{
    self,
    path::{Path, PathBuf},
    tree::Node,
};
use snafu::ResultExt;
use spin::Mutex;
use time::{Duration, OffsetDateTime};

use super::{term::Term, *};
use crate::snake::snake_game;

//

pub struct Shell {
    term: Term,
    current_dir: PathBuf,
    cmdbuf: Arc<Mutex<String>>,
    last: String,
}

//

impl Shell {
    pub fn new(term: Term) -> Self {
        Self {
            term,
            current_dir: PathBuf::new("/"),
            cmdbuf: <_>::default(),
            last: <_>::default(),
        }
    }

    pub fn init(&mut self) {
        _ = self.splash_cmd(None);
        self.prompt();
        self.term.flush();
    }

    pub async fn input(&mut self, ev: KeyboardEvent) {
        let cmdbuf = self.cmdbuf.clone();
        let mut cmdbuf = cmdbuf.lock();

        let Some(ev) = ev.unicode else {
            return;
        };

        if ev == '\n' {
            _ = writeln!(self.term);
            if let Err(err) = self.run_line(&cmdbuf).await {
                _ = writeln!(self.term, "{err}");
            };
            self.last.clear();
            _ = write!(self.last, "{cmdbuf}");
            cmdbuf.clear();
            self.prompt();
        } else if ev == '\t' {
            cmdbuf.clear();
            _ = write!(cmdbuf, "{}", self.last);
            self.prompt();
            self.term.write_bytes(cmdbuf.as_bytes());
            /* let skip = if self.term.cursor.0 % 4 == 0 {
                4
            } else {
                self.term.cursor.0 % 4
            };
            for _ in 0..skip {
                self.term.write_byte(b' ');
                cmdbuf.push(' ');
            } */
        } else if ev == '\u{8}' {
            if cmdbuf.pop().is_some() {
                self.term.cursor_prev();
                let cursor = self.term.cursor;
                self.term.write_byte(b' ');
                self.term.cursor = cursor;
            }
        } else {
            _ = write!(self.term, "{ev}");
            cmdbuf.push(ev);
        }

        self.term.flush();
    }

    fn prompt(&mut self) {
        _ = write!(self.term, "\n[kshell {}]# ", self.current_dir.as_str());
    }

    async fn run_line(&mut self, line: &str) -> Result<()> {
        let (cmd, args) = line
            .split_once(' ')
            .map(|(cmd, args)| (cmd, Some(args)))
            .unwrap_or((line, None));

        match cmd {
            "splash" => self.splash_cmd(args)?,
            "pwd" => self.pwd_cmd(args)?,
            "cd" => self.cd_cmd(args)?,
            "ls" => self.ls_cmd(args)?,
            "cat" => self.cat_cmd(args)?,
            "date" => self.date_cmd(args)?,
            "mem" => self.mem_cmd(args)?,
            "sleep" => self.sleep_cmd(args).await?,
            "draw" => self.draw_cmd(args)?,
            "kbl" => self.kbl_cmd(args)?,
            "touch" => self.touch_cmd(args)?,
            "rand" => self.rand_cmd(args)?,
            "snake" => self.snake_cmd(args).await?,
            "help" => self.help_cmd(args)?,
            "clear" => {
                self.term.clear();
            }
            "" => self.term.write_byte(b'\n'),
            other => {
                _ = writeln!(self.term, "unknown command {other}");
                self.help_cmd(None)?;
            }
        }

        Ok(())
    }

    fn splash_cmd(&mut self, _: Option<&str>) -> Result<()> {
        use hyperion_kernel_info::{BUILD_REV, BUILD_TIME, NAME, VERSION};
        // _ = writeln!(self.term, "{SPLASH}");
        _ = writeln!(
            self.term,
            "Welcome to {NAME} - {VERSION} (built {BUILD_TIME} build [{BUILD_REV}])"
        );
        Ok(())
    }

    fn pwd_cmd(&mut self, _: Option<&str>) -> Result<()> {
        _ = writeln!(self.term, "{}", self.current_dir.as_str());
        Ok(())
    }

    fn cd_cmd(&mut self, args: Option<&str>) -> Result<()> {
        let resource = Path::from_str(args.unwrap_or("/")).to_absolute(&self.current_dir);
        self.current_dir = resource.into_owned();

        Ok(())
    }

    fn ls_cmd(&mut self, args: Option<&str>) -> Result<()> {
        let resource = Path::from_str(args.unwrap_or(".")).to_absolute(&self.current_dir);
        let resource = resource.as_ref();

        let dir = hyperion_vfs::get_node(resource, false).context(IoSnafu { resource })?;

        match dir {
            Node::File(_) => {
                if let Some(file_name) = resource.file_name() {
                    _ = writeln!(self.term, "{file_name}");
                } else {
                    return Err(Error::NamelessFile);
                }
            }
            Node::Directory(dir) => {
                let mut dir = dir.lock();
                for entry in dir.nodes().context(IoSnafu { resource })?.iter() {
                    _ = writeln!(self.term, "{entry}");
                }
            }
        }

        Ok(())
    }

    fn cat_cmd(&mut self, args: Option<&str>) -> Result<()> {
        let resource = Path::from_str(args.unwrap_or(".")).to_absolute(&self.current_dir);
        let resource = resource.as_ref();

        let file = hyperion_vfs::get_file(resource, false, false).context(IoSnafu { resource })?;
        let file = file.lock();

        let mut at = 0usize;
        let mut buf = [0u8; 16];
        loop {
            let _addr = (&*file) as *const _ as *const () as u64;
            let read = file.read(at, &mut buf).context(IoSnafu { resource })?;

            if read == 0 {
                break;
            }
            at += read;

            for byte in buf {
                self.term.write_byte(byte);
            }
            self.term.write_byte(b'\n');
        }

        Ok(())
    }

    fn date_cmd(&mut self, _: Option<&str>) -> Result<()> {
        let resource = Path::from_str("/dev/rtc");

        let file = hyperion_vfs::get_file(resource, false, false).context(IoSnafu { resource })?;
        let file = file.lock();

        let mut timestamp = [0u8; 8];
        file.read_exact(0, &mut timestamp)
            .context(IoSnafu { resource })?;

        let date = OffsetDateTime::from_unix_timestamp(i64::from_le_bytes(timestamp));

        _ = writeln!(self.term, "{date:?}");

        Ok(())
    }

    fn mem_cmd(&mut self, _: Option<&str>) -> Result<()> {
        let pfa = PageFrameAllocator::get();
        let used = pfa.used_mem();
        let usable = pfa.usable_mem();
        _ = writeln!(
            self.term,
            "Mem:\n - total: {}B\n - used: {}B ({:3.1}%)",
            usable.postfix_binary(),
            used.postfix_binary(),
            used as f64 / usable as f64 * 100.0
        );

        Ok(())
    }

    async fn sleep_cmd(&mut self, seconds: Option<&str>) -> Result<()> {
        let seconds: u64 = seconds
            .map(|s| s.parse::<u8>())
            .transpose()
            .context(ParseSnafu {})?
            .unwrap_or(1) as _;

        sleep(Duration::seconds(seconds as _)).await;

        Ok(())
    }

    fn draw_cmd(&mut self, args: Option<&str>) -> Result<()> {
        let mut args = args.unwrap_or("").split(' ').filter(|arg| !arg.is_empty());
        let mode = args.next().unwrap_or("");

        fn next_int<'a>(
            term: &mut Term,
            args: &mut impl Iterator<Item = &'a str>,
        ) -> Option<usize> {
            let Some(arg) = args.next() else {
                _ = writeln!(term, "unexpected EOF, expected number");
                return None;
            };

            match arg.parse() {
                Err(err) => {
                    _ = writeln!(term, "failed to parse number: {err}");
                    None
                }
                Ok(n) => Some(n),
            }
        }

        fn next_color<'a>(
            term: &mut Term,
            args: &mut impl Iterator<Item = &'a str>,
        ) -> Option<Color> {
            let Some(arg) = args.next() else {
                _ = writeln!(term, "unexpected EOF, expected color");
                return None;
            };

            if let Some(col) = Color::from_hex(arg) {
                Some(col)
            } else {
                _ = writeln!(term, "invalid color hex code");
                None
            }
        }

        match mode {
            "rect" => {
                let Some(x) = next_int(&mut self.term, &mut args) else { return Ok(()) };
                let Some(y) = next_int(&mut self.term, &mut args) else { return Ok(()) };
                let Some(mut w) = next_int(&mut self.term, &mut args) else { return Ok(()) };
                let Some(mut h) = next_int(&mut self.term, &mut args) else { return Ok(()) };
                let Some(col) = next_color(&mut self.term, &mut args) else { return Ok(()) };

                let mut fbo = Framebuffer::get().unwrap().lock();
                if x > fbo.width || y > fbo.height || w == 0 || h == 0 {
                    return Ok(());
                }
                w = w.min(fbo.width - x);
                h = h.min(fbo.height - y);
                fbo.fill(x, y, w, h, col);

                Ok(())
            }
            "line" => {
                // TODO:
                _ = writeln!(self.term, "todo");
                Ok(())
            }
            "" => {
                _ = writeln!(self.term, "specify mode [one of: rect, line]");
                Ok(())
            }
            _ => {
                _ = writeln!(
                    self.term,
                    "invalid mode `{mode}` [should be one of: rect, line]"
                );
                Ok(())
            }
        }
    }

    fn kbl_cmd(&mut self, args: Option<&str>) -> Result<()> {
        let name = args.unwrap_or("us");
        if set_layout(name).is_none() {
            _ = writeln!(self.term, "invalid layout `{name}`");
            _ = writeln!(self.term, "available layouts(s): `{:?}`", layouts());
        }

        Ok(())
    }

    fn touch_cmd(&mut self, args: Option<&str>) -> Result<()> {
        let Some(file) = args else {
            _ = writeln!(
                self.term,
                "missing file arg"
                );
            return Ok(())
        };

        let resource = Path::from_str(file).to_absolute(&self.current_dir);
        let resource = resource.as_ref();

        hyperion_vfs::get_file(file, true, true).context(IoSnafu { resource })?;

        Ok(())
    }

    fn rand_cmd(&mut self, _: Option<&str>) -> Result<()> {
        let mut rng = hyperion_random::next_secure_rng().ok_or(Error::InsecurePrng)?;
        _ = writeln!(self.term, "{}", rng.gen::<u64>());

        Ok(())
    }

    async fn snake_cmd(&mut self, _: Option<&str>) -> Result<()> {
        snake_game(&mut self.term).await
    }

    fn help_cmd(&mut self, _: Option<&str>) -> Result<()> {
        _ = writeln!(self.term, "available commands:\nsplash, pwd, cd, ls, cat, date, mem, sleep, draw, kbl, touch, rand, snake, help, clear");

        Ok(())
    }
}
