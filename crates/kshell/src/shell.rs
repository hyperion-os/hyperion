use alloc::{
    borrow::Cow,
    format,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::{fmt::Write, sync::atomic::Ordering, time::Duration};

use futures_util::stream::select;
use hyperion_color::Color;
use hyperion_cpu_id::cpu_count;
use hyperion_driver_acpi::apic::ApicId;
use hyperion_futures::timer::ticks;
use hyperion_instant::Instant;
use hyperion_kernel_impl::{PipeInput, PipeOutput, VFS_ROOT};
use hyperion_keyboard::{
    event::{ElementState, KeyCode, KeyboardEvent},
    layouts, set_layout,
};
use hyperion_mem::pmm;
use hyperion_num_postfix::NumberPostfix;
use hyperion_random::Rng;
use hyperion_scheduler::{
    idle,
    ipc::pipe::channel,
    schedule, spawn,
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
            "draw" => self.draw_cmd(args)?,
            "kbl" => self.kbl_cmd(args)?,
            "rand" => self.rand_cmd(args)?,
            "snake" => self.snake_cmd(args).await?,
            "help" => self.help_cmd(args)?,
            "modeltest" => self.modeltest_cmd(args).await?,
            "lapic_id" => self.lapic_id_cmd(args)?,
            "cpu_id" => self.cpu_id_cmd(args)?,
            "ps" => self.ps_cmd(args)?,
            "nproc" => self.nproc_cmd(args)?,
            "top" => self.top_cmd(args)?,
            "send" => self.send_cmd(args)?,
            "kill" => self.kill_cmd(args)?,
            "exit" => return Ok(None),
            "clear" => {
                self.term.clear();
            }
            "" => self.term.write_byte(b'\n'),
            other => {
                let path = format!("/bin/{other}");
                let elf = self.load_elf(&path)?;

                self.run_cmd_from(path.into(), elf.into(), args).await?;
            }
        }

        Ok(Some(()))
    }

    fn load_elf(&self, path: &str) -> Result<Vec<u8>> {
        let mut elf = Vec::new();
        let Ok(bin) = VFS_ROOT.find_file(path, false, false) else {
            return Err(Error::Other {
                msg: "unknown command {path}".into(),
            });
        };

        let bin = bin.lock_arc();

        loop {
            let mut buf = [0; 64];
            let len = bin.read(elf.len(), &mut buf).unwrap();
            elf.extend_from_slice(&buf[..len]);
            if len == 0 {
                break;
            }
        }

        Ok(elf)
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
                let Some(x) = next_int(&mut self.term, &mut args) else {
                    return Ok(());
                };
                let Some(y) = next_int(&mut self.term, &mut args) else {
                    return Ok(());
                };
                let Some(mut w) = next_int(&mut self.term, &mut args) else {
                    return Ok(());
                };
                let Some(mut h) = next_int(&mut self.term, &mut args) else {
                    return Ok(());
                };
                let Some(col) = next_color(&mut self.term, &mut args) else {
                    return Ok(());
                };

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

    fn rand_cmd(&mut self, _: Option<&str>) -> Result<()> {
        let mut rng = hyperion_random::next_secure_rng().ok_or(Error::InsecurePrng)?;
        _ = writeln!(self.term, "{}", rng.gen::<u64>());

        Ok(())
    }

    async fn snake_cmd(&mut self, _: Option<&str>) -> Result<()> {
        snake_game(&mut self.term).await
    }

    fn help_cmd(&mut self, _: Option<&str>) -> Result<()> {
        _ = writeln!(self.term, "available built-in shell commands:\nsplash, pwd, cd, date, mem, draw, kbl, rand, snake, help, modeltest, run, lapic_id, cpu_id, ps, nproc, top, send, kill, exit, clear");

        Ok(())
    }

    async fn modeltest_cmd(&mut self, _: Option<&str>) -> Result<()> {
        use glam::{Mat4, Vec3, Vec3Swizzles, Vec4, Vec4Swizzles};

        fn draw_line(fbo: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
            let dx = x0.abs_diff(x1);
            let dy = y0.abs_diff(y1);

            if dx > dy {
                for x in x0.min(x1)..=x0.max(x1) {
                    let t = (x - x0) as f32 / (x1 - x0) as f32;
                    let y = (t * (y1 - y0) as f32) as i32 + y0;

                    fbo.pixel(x as _, y as _, color);
                }
            } else {
                for y in y0.min(y1)..=y0.max(y1) {
                    let t = (y - y0) as f32 / (y1 - y0) as f32;
                    let x = (t * (x1 - x0) as f32) as i32 + x0;

                    fbo.pixel(x as _, y as _, color);
                }
            }
        }

        fn draw_cube(fbo: &mut Framebuffer, x: i32, y: i32, model: Mat4, s: f32, color: Color) {
            let mat = Mat4::perspective_rh(0.005, 1.0, 0.01, 600.0)
                * Mat4::look_at_rh(Vec3::new(0.0, 0.0, 300.0), Vec3::ZERO, Vec3::NEG_Y)
                * model;

            let mut translated_line = |a: Vec3, b: Vec3| {
                let a = mat * Vec4::from((a, 1.0));
                let b = mat * Vec4::from((b, 1.0));
                let a = (a.xyz() / a.w).xy().as_ivec2();
                let b = (b.xyz() / b.w).xy().as_ivec2();

                draw_line(fbo, a.x + x, a.y + y, b.x + x, b.y + y, color);
            };

            translated_line(Vec3::new(-s, -s, -s), Vec3::new(s, -s, -s));
            translated_line(Vec3::new(-s, s, -s), Vec3::new(s, s, -s));
            translated_line(Vec3::new(-s, -s, s), Vec3::new(s, -s, s));
            translated_line(Vec3::new(-s, s, s), Vec3::new(s, s, s));

            translated_line(Vec3::new(-s, -s, -s), Vec3::new(-s, s, -s));
            translated_line(Vec3::new(s, -s, -s), Vec3::new(s, s, -s));
            translated_line(Vec3::new(-s, -s, s), Vec3::new(-s, s, s));
            translated_line(Vec3::new(s, -s, s), Vec3::new(s, s, s));

            translated_line(Vec3::new(-s, -s, -s), Vec3::new(-s, -s, s));
            translated_line(Vec3::new(s, -s, -s), Vec3::new(s, -s, s));
            translated_line(Vec3::new(-s, s, -s), Vec3::new(-s, s, s));
            translated_line(Vec3::new(s, s, -s), Vec3::new(s, s, s));
        }

        let mid_x = ((self.term.size.0 * CHAR_SIZE.0 as usize) / 2) as i32;
        let mid_y = ((self.term.size.1 * CHAR_SIZE.1 as usize) / 2) as i32;
        let mut a = 0.0f32;

        let ticks = ticks(time::Duration::milliseconds(10)).map(|_| None);
        let esc = KeyboardEvents.map(Some);
        let mut events = select(ticks, esc);

        loop {
            let Some(fbo) = Framebuffer::get() else {
                break;
            };
            let mut fbo = fbo.lock();

            let red = Mat4::from_rotation_y(a);
            let blue = Mat4::from_rotation_y(a * 2.0);
            draw_cube(&mut fbo, mid_x, mid_y, red, 100.0, Color::RED);
            draw_cube(&mut fbo, mid_x, mid_y, blue, 80.0, Color::BLUE);

            let stop = matches!(
                events.next().await,
                Some(Some(KeyboardEvent {
                    keycode: KeyCode::Escape,
                    ..
                }))
            );

            let red = Mat4::from_rotation_y(a);
            let blue = Mat4::from_rotation_y(a * 2.0);
            draw_cube(&mut fbo, mid_x, mid_y, red, 100.0, Color::BLACK);
            draw_cube(&mut fbo, mid_x, mid_y, blue, 80.0, Color::BLACK);
            a += 0.01;

            if stop {
                break;
            }
        }

        Ok(())
    }

    async fn run_cmd_from(
        &mut self,
        name: Cow<'static, str>,
        elf: Cow<'static, [u8]>,
        args: Option<&str>,
    ) -> Result<()> {
        let args = args.map(String::from);

        // TODO: HACK:
        let is_doom = name.ends_with("doom");

        // setup STDIO
        let (stdin_tx, stdin_rx) = channel();
        let (stdout_tx, stdout_rx) = channel();
        let (stderr_tx, stderr_rx) = channel();

        // hacky blocking channel -> async channel stuff
        let (o_tx, o_rx) = hyperion_futures::mpmc::channel();

        // stdout -> terminal
        let o_tx_2 = o_tx.clone();
        spawn(move || {
            loop {
                let mut buf = [0; 128];
                let Ok(len) = stdout_rx.recv_slice(&mut buf) else {
                    debug!("end of stream");
                    break;
                };
                let Ok(str) = core::str::from_utf8(&buf[..len]) else {
                    debug!("invalid utf8");
                    break;
                };
                o_tx_2.send(Some(str.to_string()));
            }

            o_tx_2.send(None);
        });

        // stderr -> kernel logs
        spawn(move || loop {
            let mut buf = [0; 128];
            let Ok(len) = stderr_rx.recv_slice(&mut buf) else {
                debug!("end of stream");
                break;
            };
            let Ok(str) = core::str::from_utf8(&buf[..len]) else {
                debug!("invalid utf8");
                break;
            };

            print!("{str}");
        });

        // spawn the new process
        schedule(move || {
            // set its name
            let name = name.as_ref();
            hyperion_scheduler::rename(name);

            // setup the STDIO
            hyperion_kernel_impl::push_file(hyperion_kernel_impl::FileInner {
                file_ref: Arc::new(lock_api::Mutex::new(PipeOutput(stdin_rx))) as _,
                position: 0,
            });
            hyperion_kernel_impl::push_file(hyperion_kernel_impl::FileInner {
                file_ref: Arc::new(lock_api::Mutex::new(PipeInput(stdout_tx))) as _,
                position: 0,
            });
            hyperion_kernel_impl::push_file(hyperion_kernel_impl::FileInner {
                file_ref: Arc::new(lock_api::Mutex::new(PipeInput(stderr_tx))) as _,
                position: 0,
            });

            // load and exec the binary
            let args: Vec<&str> = [name] // TODO: actually load binaries from vfs
                .into_iter()
                .chain(args.as_deref().iter().flat_map(|args| args.split(' ')))
                .collect();
            let args = &args[..];

            hyperion_log::trace!("spawning \"{name}\" with args {args:?}");

            let loader = hyperion_loader::Loader::new(elf.as_ref());

            loader.load();

            if loader.enter_userland(args).is_none() {
                hyperion_log::debug!("entry point missing");
            }
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
                    {
                        break;
                    }

                    // if let Some(unicode) = unicode {
                    //     // _ = write!(self.term, "{unicode}");
                    //     // self.term.flush();

                    //     let mut str = [0; 4];

                    //     let str = unicode.encode_utf8(&mut str);

                    //     // TODO: buffering
                    //     if let Err(err) = stdin_tx.send_slice(str.as_bytes()) {
                    //         break;
                    //     }
                    // }

                    // TODO: proper raw keyboard input
                    #[derive(serde::Serialize, serde::Deserialize)]
                    struct KeyboardEventSer {
                        // pub scancode: u8,
                        state: u8,
                        keycode: u8,
                        unicode: Option<char>,
                    }

                    let ev = serde_json::to_string(&KeyboardEventSer {
                        state: state as u8,
                        keycode: keycode as u8,
                        unicode,
                    })
                    .unwrap();
                    // hyperion_log::debug!("sending {ev}");
                    if stdin_tx.send_slice(ev.as_bytes()).is_err() {
                        // hyperion_log::debug!("stdin closed");
                        break;
                    }
                    if stdin_tx.send_slice("\n".as_bytes()).is_err() {
                        // hyperion_log::debug!("stdin closed");
                        break;
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
                    break;
                }
                None => break,
            }
        }

        hyperion_log::debug!("done");

        // stdin_tx.close();

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

    fn send_cmd(&mut self, args: Option<&str>) -> Result<()> {
        let Some(args) = args else {
            // _ = writeln!(self.term, "expected arg: PID");
            return Err(Error::Other {
                msg: "expected arg: PID".into(),
            });
        };

        let Some((pid, data)) = args.split_once(' ') else {
            return Err(Error::Other {
                msg: "expected arg: DATA".into(),
            });
        };

        let pid: usize = match pid.parse() {
            Ok(pid) => pid,
            Err(err) => {
                return Err(Error::Other {
                    msg: format!("failed to parse PID: {err}"),
                });
            }
        };

        let data = data.replace("\\n", "\n");

        if let Err(err) =
            hyperion_scheduler::send(hyperion_scheduler::task::Pid::new(pid), data.as_bytes())
        {
            return Err(Error::Other {
                msg: format!("failed send data: {err}"),
            });
        };

        Ok(())
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
