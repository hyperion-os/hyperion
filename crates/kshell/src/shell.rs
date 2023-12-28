use alloc::{
    format,
    string::{String, ToString},
    sync::Arc,
};
use core::{fmt::Write, sync::atomic::Ordering};

use anyhow::anyhow;
use futures_util::stream::select;
use hyperion_cpu_id::cpu_count;
use hyperion_driver_acpi::apic::ApicId;
use hyperion_instant::Instant;
use hyperion_kernel_impl::{FileDescData, FileDescriptor, VFS_ROOT};
use hyperion_keyboard::{
    event::{ElementState, KeyCode, KeyboardEvent},
    layouts, set_layout,
};
use hyperion_mem::pmm;
use hyperion_num_postfix::NumberPostfix;
use hyperion_scheduler::{
    idle,
    ipc::pipe::pipe,
    spawn,
    task::{processes, Pid, TASKS_READY, TASKS_RUNNING, TASKS_SLEEPING},
};
use hyperion_vfs::{
    self,
    path::{Path, PathBuf},
};
use snafu::ResultExt;
use spin::Mutex;
use time::OffsetDateTime;

use super::{term::Term, *};
use crate::{
    cmd::{Command, NULL_DEV},
    snake::snake_game,
};

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

    pub fn into_inner(self) -> Term {
        self.term
    }

    pub fn init(&mut self) {
        _ = self.splash_cmd(None);
        self.prompt();
        self.term.flush();
    }

    pub async fn input(&mut self, ev: KeyboardEvent) -> Option<()> {
        let cmdbuf = self.cmdbuf.clone();
        let mut cmdbuf = cmdbuf.lock();

        let Some(ev) = ev.unicode else {
            return Some(());
        };

        if ev == '\n' {
            _ = writeln!(self.term);
            match self.run_line(&cmdbuf).await {
                Ok(v) => {
                    v?;
                }
                Err(err) => {
                    _ = writeln!(self.term, "{err}");
                }
            }
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

        Some(())
    }

    fn prompt(&mut self) {
        _ = write!(self.term, "\n[kshell {}]# ", self.current_dir.as_str());
    }

    async fn run_line(&mut self, line: &str) -> Result<Option<()>> {
        let line = line.trim();

        let (cmd, args) = line
            .split_once(' ')
            .map(|(cmd, args)| (cmd, Some(args)))
            .unwrap_or((line, None));

        match cmd {
            "splash" => self.splash_cmd(args)?,
            "pwd" => self.pwd_cmd(args)?,
            "cd" => self.cd_cmd(args)?,
            "date" => self.date_cmd(args)?,
            "mem" => self.mem_cmd(args)?,
            "kbl" => self.kbl_cmd(args)?,
            "snake" => self.snake_cmd(args).await?,
            "help" => self.help_cmd(args)?,
            "lapic_id" => self.lapic_id_cmd(args)?,
            "cpu_id" => self.cpu_id_cmd(args)?,
            "ps" => self.ps_cmd(args)?,
            "nproc" => self.nproc_cmd(args)?,
            "top" => self.top_cmd(args)?,
            "kill" => self.kill_cmd(args)?,
            "exit" => return Ok(None),
            "clear" => self.term.clear(),
            "" => self.term.write_byte(b'\n'),
            _ => self.cmd_line(line).await.map_err(|err| Error::Other {
                msg: err.to_string(),
            })?,
        }

        Ok(Some(()))
    }

    async fn cmd_line(&mut self, line: &str) -> anyhow::Result<()> {
        let mut is_doom = false;

        let (stdin_tx, stdin_rx) = pipe().split();
        let (stderr_tx, stderr_rx) = pipe().split();

        let stderr = Arc::new(stderr_tx);
        let mut stdin = Arc::new(stdin_rx) as _;
        for part in line.split('|') {
            let mut redirects = part.split('>');

            let program = redirects.next().unwrap_or(part).trim();
            let (program, args) = program
                .split_once(' ')
                .map(|(cmd, args)| (cmd, Some(args)))
                .unwrap_or((program, None));

            let program = if program.starts_with('/') {
                program.to_string()
            } else {
                format!("/bin/{program}")
            };

            let args = args.map(|v| v.split(' ')).into_iter().flatten();

            // TODO: HACK:
            is_doom |= program.ends_with("doom");

            let mut cmd = Command::new(program);
            cmd.args(args).stdin(stdin).stderr(stderr.clone());

            if let Some(output_file) = redirects.last() {
                let stdout_tx = Arc::new(
                    FileDescData::open(output_file.trim())
                        .map_err(|err| anyhow!("couldn't open file `{output_file:?}`: {err}"))?,
                );

                // spawn the new process
                cmd.stdout(stdout_tx).spawn()?;

                stdin = NULL_DEV.clone();
            } else {
                let (stdout_tx, stdout_rx) = pipe().split();

                // spawn the new process
                cmd.stdout(Arc::new(stdout_tx)).spawn()?;

                stdin = Arc::new(stdout_rx);
            };
        }

        // hacky blocking channel -> async channel stuff
        let (o_tx, o_rx) = hyperion_futures::mpmc::channel();
        // last program's stdout (stdin) -> terminal
        let o_tx_2 = o_tx.clone();
        spawn(move || {
            loop {
                let mut buf = [0; 128];
                let Ok(len) = stderr_rx.read(&mut buf) else {
                    trace!("end of stream");
                    break;
                };
                if len == 0 {
                    trace!("end of stream");
                    break;
                }
                let Ok(str) = core::str::from_utf8(&buf[..len]) else {
                    trace!("invalid utf8");
                    break;
                };
                o_tx_2.send(Some(str.to_string()));
            }

            o_tx_2.send(None);
        });
        spawn(move || {
            loop {
                let mut buf = [0; 128];
                let Ok(len) = stdin.read(&mut buf) else {
                    trace!("end of stream");
                    break;
                };
                if len == 0 {
                    trace!("end of stream");
                    break;
                }
                let Ok(str) = core::str::from_utf8(&buf[..len]) else {
                    trace!("invalid utf8");
                    break;
                };
                o_tx.send(Some(str.to_string()));
            }

            o_tx.send(None);
        });

        // start sending keyboard events to the process and read stdout into the terminal
        let mut events = select(KeyboardEvents.map(Ok), o_rx.race_stream().map(Err));

        let mut l_ctrl_held = false;
        loop {
            // hyperion_log::debug!("Waiting for events ...");
            let ev = events.next().await;
            // hyperion_log::debug!("Event {ev:?}");

            match ev {
                Some(Ok(KeyboardEvent {
                    state,
                    keycode,
                    unicode,
                })) => {
                    if state == ElementState::PressHold && keycode == KeyCode::LControl {
                        l_ctrl_held = true;
                    }
                    if state == ElementState::PressRelease && keycode == KeyCode::LControl {
                        l_ctrl_held = false;
                    }

                    if state != ElementState::PressRelease
                        && (keycode == KeyCode::C || keycode == KeyCode::D)
                        && l_ctrl_held
                        && !is_doom
                    // no ctrl+c / ctrl+d in raw mode
                    {
                        trace!("ctrl+C/D");
                        break;
                    }

                    // TODO: raw mode
                    if is_doom {
                        // TODO: proper raw keyboard input
                        #[derive(serde::Serialize, serde::Deserialize)]
                        struct KeyboardEventSer {
                            // pub scancode: u8,
                            state: u8,
                            keycode: u8,
                            unicode: Option<char>,
                        }

                        if keycode == KeyCode::CapsLock {
                            if stdin_tx.send_slice(b"\n").is_err() {
                                trace!("stdin closed");
                                break;
                            }
                            continue;
                        }

                        let mut ev = serde_json::to_string(&KeyboardEventSer {
                            state: state as u8,
                            keycode: keycode as u8,
                            unicode,
                        })
                        .unwrap();
                        ev.push('\n');
                        // debug!("sending: {ev:?}");
                        if stdin_tx.send_slice(ev.as_bytes()).is_err() {
                            trace!("stdin closed");
                            break;
                        }
                    } else if let Some(unicode) = unicode {
                        _ = write!(self.term, "{unicode}");
                        self.term.flush();

                        let mut str = [0; 4];

                        let str = unicode.encode_utf8(&mut str);

                        // TODO: buffering
                        if stdin_tx.send_slice(str.as_bytes()).is_err() {
                            break;
                        }
                    }
                }
                Some(Err(Some(s))) => {
                    // TODO: HACK: doom locks the framebuffer and flushing here would deadlock,
                    // as kshell cannot send any keyboard input anymore
                    if is_doom {
                        continue;
                    }

                    _ = write!(self.term, "{s}");
                    self.term.flush();
                }
                Some(Err(None)) => {
                    // _ = write!(self.term, "got EOI");
                    // self.term.flush();
                    trace!("EOI");
                    break;
                }
                None => {
                    trace!("NONE");
                    break;
                }
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

    fn date_cmd(&mut self, _: Option<&str>) -> Result<()> {
        let resource = Path::from_str("/dev/rtc");

        let file = VFS_ROOT
            .find_file(resource, false, false)
            .context(IoSnafu { resource })?;
        let file = file.lock();

        let mut timestamp = [0u8; 8];
        file.read_exact(0, &mut timestamp)
            .context(IoSnafu { resource })?;

        let date = OffsetDateTime::from_unix_timestamp(i64::from_le_bytes(timestamp));

        _ = writeln!(self.term, "{date:?}");

        Ok(())
    }

    fn mem_cmd(&mut self, _: Option<&str>) -> Result<()> {
        let used = pmm::PFA.used_mem().postfix_binary();
        let usable = pmm::PFA.usable_mem().postfix_binary();
        let total = pmm::PFA.total_mem().postfix_binary();

        let p = used.into_inner() as f64 / usable.into_inner() as f64 * 100.0;

        _ = writeln!(
            self.term,
            "Mem:\n - total: {total}B\n - usable: {usable}B\n - used: {used}B ({p:3.1}%)",
        );

        Ok(())
    }

    fn kbl_cmd(&mut self, args: Option<&str>) -> Result<()> {
        let name = args.unwrap_or("us");
        if set_layout(name).is_none() {
            _ = writeln!(self.term, "invalid layout `{name}`");
            _ = writeln!(self.term, "available layouts(s): `{:?}`", layouts());
        }

        Ok(())
    }

    async fn snake_cmd(&mut self, _: Option<&str>) -> Result<()> {
        snake_game(&mut self.term).await
    }

    fn help_cmd(&mut self, _: Option<&str>) -> Result<()> {
        _ = writeln!(self.term, "available built-in shell commands:\nsplash, pwd, cd, date, mem, kbl, snake, help, lapic_id, cpu_id, ps, nproc, top, kill, exit, clear");

        Ok(())
    }

    fn lapic_id_cmd(&mut self, _args: Option<&str>) -> Result<()> {
        _ = writeln!(self.term, "{:?}", ApicId::current());
        Ok(())
    }

    fn cpu_id_cmd(&mut self, _args: Option<&str>) -> Result<()> {
        _ = writeln!(self.term, "{:?}", hyperion_cpu_id::cpu_id());
        Ok(())
    }

    fn ps_cmd(&mut self, _args: Option<&str>) -> Result<()> {
        let processes = processes();

        _ = writeln!(
            self.term,
            "\n{: >6} {: >7} {: >9} CMD",
            "PID", "THREADS", "TIME"
        );
        for proc in processes {
            let pid = proc.pid;
            let threads = proc.threads.load(Ordering::Relaxed);
            let time = time::Duration::nanoseconds(proc.nanos.load(Ordering::Relaxed) as _);
            // let time_h = time.whole_hours();
            let time_m = time.whole_minutes() % 60;
            let time_s = time.whole_seconds() % 60;
            let time_ms = time.whole_milliseconds() % 1000;
            let name = proc.name.read();

            _ = writeln!(
                self.term,
                "{pid: >6} {threads: >7} {time_m: >2}:{time_s:02}.{time_ms:03} {name}"
            );
        }

        Ok(())
    }

    fn nproc_cmd(&mut self, _args: Option<&str>) -> Result<()> {
        _ = writeln!(self.term, "{}", cpu_count());

        Ok(())
    }

    fn top_cmd(&mut self, _args: Option<&str>) -> Result<()> {
        let uptime = Instant::now() - Instant::new(0);

        let uptime_h = uptime.whole_hours();
        let uptime_m = uptime.whole_minutes() % 60;
        let uptime_s = uptime.whole_seconds() % 60;

        /* let tasks = hyperion_scheduler::tasks();
        let task_states = tasks.iter().map(|task| task.state.load()); */
        let tasks_running = TASKS_RUNNING.load(Ordering::Relaxed);
        let tasks_sleeping = TASKS_SLEEPING.load(Ordering::Relaxed);
        let tasks_ready = TASKS_READY.load(Ordering::Relaxed);
        let tasks_total = tasks_running + tasks_sleeping + tasks_ready;

        let mem_total = pmm::PFA.usable_mem().postfix_binary();
        let mem_free = pmm::PFA.free_mem().postfix_binary();
        let mem_used = pmm::PFA.used_mem().postfix_binary();

        _ = writeln!(self.term, "top - {uptime_h}:{uptime_m:02}:{uptime_s:02} up");
        _ = writeln!(
            self.term,
            "Tasks: {tasks_total} total, {tasks_running} running, {tasks_sleeping} sleeping, {tasks_ready} ready"
        );
        _ = writeln!(
            self.term,
            "Mem: {mem_total} total, {mem_free} free, {mem_used} used"
        );

        _ = write!(self.term, "Cpu idles: ");
        for idle in idle() {
            // round the time
            let idle = time::Duration::milliseconds(idle.whole_milliseconds() as _);
            _ = write!(self.term, "{idle}, ");
        }
        _ = writeln!(self.term);

        self.ps_cmd(None)
    }

    fn kill_cmd(&mut self, args: Option<&str>) -> Result<()> {
        let Some(arg) = args else {
            return Err(Error::Other {
                msg: "missing arg pid".to_string(),
            });
        };

        let Ok(pid) = arg.parse::<usize>() else {
            return Err(Error::Other {
                msg: "invalid arg pid".to_string(),
            });
        };

        let Some(proc) = Pid::new(pid).find() else {
            return Err(Error::Other {
                msg: "couldn't find the process".to_string(),
            });
        };

        proc.should_terminate.store(true, Ordering::SeqCst);

        Ok(())
    }
}
