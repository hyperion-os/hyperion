use crate::{
    arch,
    driver::acpi::hpet::HPET,
    mem::{pmm::PageFrameAllocator},
    util::fmt::NumberPostfix,
    vfs::{
        self,
        path::{Path, PathBuf},
        Node,
    },
    KERNEL_BUILD_REV, KERNEL_BUILD_TIME, KERNEL_NAME, KERNEL_VERSION,
};
use alloc::{borrow::ToOwned, string::String, sync::Arc};
use chrono::{TimeZone, Utc};
use core::fmt::Write;
use snafu::ResultExt;
use spin::{Mutex};


use super::{term::Term, *};

//

pub struct Shell<'fbo> {
    term: Term<'fbo>,
    current_dir: PathBuf,
    cmdbuf: Arc<Mutex<String>>,
}

//

impl<'fbo> Shell<'fbo> {
    pub fn new(term: Term<'fbo>) -> Self {
        Self {
            term,
            current_dir: PathBuf::new("/"),
            cmdbuf: <_>::default(),
        }
    }

    pub fn init(&mut self) {
        _ = self.splash_cmd(None);
        self.prompt();
        self.term.flush();
    }

    pub fn input(&mut self, ev: char) {
        let cmdbuf = self.cmdbuf.clone();
        let mut cmdbuf = cmdbuf.lock();

        if ev == '\n' {
            _ = writeln!(self.term);
            if let Err(err) = self.run_line(&cmdbuf) {
                _ = writeln!(self.term, "{err}");
            };
            cmdbuf.clear();
            self.prompt();
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

    pub fn tick(&mut self) {
        // crate::debug!("tick : {}", HPET.lock().main_counter_value());
    }

    fn prompt(&mut self) {
        _ = write!(self.term, "\n[kshell {}]# ", self.current_dir.as_str());
    }

    fn run_line(&mut self, line: &str) -> Result<()> {
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
            "sleep" => self.sleep_cmd(args)?,
            "clear" => {
                self.term.clear();
            }
            "" => self.term.write_byte(b'\n'),
            other => {
                _ = writeln!(self.term, "unknown command {other}");
            }
        }

        Ok(())
    }

    fn splash_cmd(&mut self, _: Option<&str>) -> Result<()> {
        // _ = writeln!(self.term, "{KERNEL_SPLASH}");
        _ = writeln!(self.term, "Welcome to {KERNEL_NAME} - {KERNEL_VERSION} (built {KERNEL_BUILD_TIME} build [{KERNEL_BUILD_REV}])");
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

        let dir = vfs::get_node(resource, false).with_context(|_| IoSnafu {
            resource: resource.to_owned(),
        })?;

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
                for entry in dir.nodes().with_context(|_| IoSnafu {
                    resource: resource.to_owned(),
                })? {
                    _ = writeln!(self.term, "{entry}");
                }
            }
        }

        Ok(())
    }

    fn cat_cmd(&mut self, args: Option<&str>) -> Result<()> {
        let resource = Path::from_str(args.unwrap_or(".")).to_absolute(&self.current_dir);
        let resource = resource.as_ref();

        let file = vfs::get_file(resource, false, false).with_context(|_| IoSnafu {
            resource: resource.to_owned(),
        })?;
        let file = file.lock();

        let mut at = 0usize;
        let mut buf = [0u8; 16];
        loop {
            let _addr = (&*file) as *const _ as *const () as u64;
            let read = file.read(at, &mut buf).with_context(|_| IoSnafu {
                resource: resource.to_owned(),
            })?;

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

        let file = vfs::get_file(resource, false, false).with_context(|_| IoSnafu {
            resource: resource.to_owned(),
        })?;
        let file = file.lock();

        let mut timestamp = [0u8; 8];
        file.read_exact(0, &mut timestamp)
            .with_context(|_| IoSnafu {
                resource: resource.to_owned(),
            })?;

        let date = Utc.timestamp_nanos(i64::from_le_bytes(timestamp));

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

    fn sleep_cmd(&mut self, seconds: Option<&str>) -> Result<()> {
        let seconds = seconds
            .map(|s| s.parse::<u8>())
            .transpose()
            .context(ParseSnafu {})?
            .unwrap_or(1);

        let now = HPET.lock().millis();
        while now + 1_000 * seconds as u128 >= HPET.lock().millis() {
            arch::spin_loop();
        }

        Ok(())
    }
}
