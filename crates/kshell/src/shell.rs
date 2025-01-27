use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    sync::Arc,
};
use core::{fmt::Write, str, sync::atomic::Ordering};

use anyhow::anyhow;
use futures_util::{stream::select, Stream};
use hyperion_escape::encode::{CursorDown, CursorLeft, CursorRight, CursorUp};
use hyperion_events::keyboard::{
    event::{ElementState, KeyCode, KeyboardEvent},
    layouts, set_layout,
};
use hyperion_futures::{keyboard::keyboard_events, mpmc};
use hyperion_kernel_impl::{FileDescData, FileDescriptor};
use hyperion_scheduler::{ipc::pipe::pipe, proc::Pid, spawn};
use hyperion_vfs::{self, path::PathBuf};
use spin::Mutex;

use super::{term::Term, *};
use crate::cmd::{Command, NULL_DEV};

//

enum Event {
    Keyboard(KeyboardEvent),
    Stdout(String),
}

//

pub struct Shell {
    term: Term,
    current_dir: PathBuf,
    cmdbuf: Arc<Mutex<String>>,
    last: String,

    stdout: Arc<dyn FileDescriptor>,
    events: Box<dyn Stream<Item = Event> + Send + Unpin>,
}

//

impl Shell {
    pub fn new(term: Term) -> Self {
        let (stdout_tx, stdout_rx) = pipe().split();

        // hacky blocking channel -> async channel stuff
        // program stdout -> terminal
        let (o_tx, o_rx) = hyperion_futures::mpmc::channel();

        fn try_forward(from: &impl FileDescriptor, to: &mpmc::Sender<String>) -> Option<()> {
            loop {
                // TODO: if the buffer is full, the result might not be UTF-8
                let mut buf = [0u8; 0x2000];
                let len = from.read(&mut buf).ok()?;

                if len == 0 {
                    return None;
                }

                let str = str::from_utf8(&buf[..len]).ok()?.to_string();
                // hyperion_log::info!("{str}");

                to.send(str).ok()?;
            }
        }

        fn forward(from: &impl FileDescriptor, to: mpmc::Sender<String>) {
            _ = try_forward(from, &to);
            // _ = to.send(None);
        }

        spawn(move || forward(&stdout_rx, o_tx));

        let events = select(
            keyboard_events().map(Event::Keyboard),
            o_rx.into_stream().map(Event::Stdout),
        );

        Self {
            term,
            current_dir: PathBuf::new("/"),
            cmdbuf: <_>::default(),
            last: <_>::default(),

            stdout: Arc::new(stdout_tx),
            events: Box::new(events),
        }
    }

    pub fn into_inner(self) -> Term {
        self.term
    }

    pub fn init(&mut self) {
        use hyperion_kernel_info::{BUILD_REV, BUILD_TIME, NAME, VERSION};
        _ = writeln!(
            self.term,
            "Welcome to {NAME} - {VERSION} (built {BUILD_TIME} build [{BUILD_REV}])"
        );
        self.prompt();
        self.term.flush();
    }

    pub async fn run(&mut self) -> Option<()> {
        let ev = self.events.next().await?;

        match ev {
            Event::Keyboard(ev) => self.input(ev).await?,
            Event::Stdout(ev) => {
                self.term.clear_line(); // remove the prompt
                self.term.cursor = self.term.stdout_cursor;
                _ = write!(self.term, "{ev}");

                self.prompt();
                self.term.write_bytes(self.cmdbuf.lock().as_bytes());
                self.term.flush();
            }
        }

        Some(())
    }

    pub async fn input(&mut self, ev: KeyboardEvent) -> Option<()> {
        let cmdbuf = self.cmdbuf.clone();
        let mut cmdbuf = cmdbuf.lock();

        let Some(ev) = ev.unicode else {
            return Some(());
        };

        if ev == '\n' {
            // enter
            _ = writeln!(self.term);
            self.term.stdout_cursor = self.term.cursor;
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
            // tab
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
            // backspace
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
        self.term.stdout_cursor = self.term.cursor;
        _ = write!(self.term, "\n[kshell {}]# ", self.current_dir.as_str());
    }

    async fn run_line(&mut self, line: &str) -> anyhow::Result<Option<()>> {
        let line = line.trim();

        let (cmd, args) = line
            .split_once(' ')
            .map(|(cmd, args)| (cmd, Some(args)))
            .unwrap_or((line, None));

        match cmd {
            "kbl" => self.kbl_cmd(args)?,
            "help" => self.help_cmd(args)?,
            "kill" => self.kill_cmd(args)?,
            "exit" => return Ok(None),
            "clear" => self.term.clear(),
            "lspci" => self.lspci_cmd(args)?,
            "" => self.term.write_byte(b'\n'),
            _ => self.cmd_line(line).await?,
        }

        Ok(Some(()))
    }

    async fn cmd_line(&mut self, line: &str) -> anyhow::Result<()> {
        let mut is_doom = false;

        // prepare shared stderr, cli input (keyboard) and cli output (term)
        let (stdin_tx, stdin_rx) = pipe().split();
        let stderr = self.stdout.clone();
        let mut stdin = Arc::new(stdin_rx) as _;

        // stop stdin reading when it closes
        let (closed_tx, closed_rx) = hyperion_futures::mpmc::channel();

        // launch all cmds
        let mut part_iter = line.split('|').peekable();
        while let Some(part) = part_iter.next() {
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

            let stdout = if let Some(output_file) = redirects.last() {
                // stdout is redirected to a file
                let stdout_tx = FileDescData::open(output_file.trim())
                    .map_err(|err| anyhow!("couldn't open file `{output_file:?}`: {err}"))?;
                stdin = NULL_DEV.clone();
                Arc::new(stdout_tx)
            } else if part_iter.peek().is_none() {
                // last cmd part uses the shell's shared pipe
                cmd.on_close(closed_tx.clone());
                stdin = NULL_DEV.clone();
                stderr.clone()
            } else {
                // stdout is redirected to the next program's stdin
                let (stdout_tx, stdout_rx) = pipe().split();
                stdin = Arc::new(stdout_rx);
                Arc::new(stdout_tx)
            };

            // spawn the new process
            cmd.stdout(stdout).spawn()?;
        }

        // start sending keyboard events to the process and read stdout into the terminal
        let mut l_ctrl_held = false;
        let mut events = select(
            self.events.as_mut().map(Some),
            closed_rx.into_stream().map(|_| None),
        );
        while let Some(Some(ev)) = events.next().await {
            match ev {
                Event::Keyboard(KeyboardEvent {
                    state,
                    keycode,
                    unicode,
                }) => {
                    if state == ElementState::Pressed && keycode == KeyCode::LControl {
                        l_ctrl_held = true;
                    }
                    if state == ElementState::Released && keycode == KeyCode::LControl {
                        l_ctrl_held = false;
                    }

                    if state == ElementState::Pressed
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
                        // _ = write!(self.term, "{unicode}");
                        // self.term.flush();

                        let mut str = [0; 4];

                        let str = unicode.encode_utf8(&mut str);

                        // TODO: buffering
                        if stdin_tx.send_slice(str.as_bytes()).is_err() {
                            break;
                        }
                    } else {
                        if state != ElementState::Pressed {
                            continue;
                        }

                        let escape_bytes = match keycode {
                            KeyCode::ArrowUp => format!("{}", CursorUp(1)),
                            KeyCode::ArrowDown => format!("{}", CursorDown(1)),
                            KeyCode::ArrowLeft => format!("{}", CursorLeft(1)),
                            KeyCode::ArrowRight => format!("{}", CursorRight(1)),
                            _ => continue,
                        };

                        if stdin_tx.send_slice(escape_bytes.as_bytes()).is_err() {
                            break;
                        }
                    }
                }
                Event::Stdout(s) => {
                    // TODO: HACK: doom locks the framebuffer and flushing here would deadlock,
                    // as kshell cannot send any keyboard input anymore
                    if is_doom {
                        continue;
                    }

                    self.term.cursor = self.term.stdout_cursor;
                    _ = write!(self.term, "{s}");
                    self.term.stdout_cursor = self.term.cursor;
                    self.term.flush();
                }
            }
        }

        Ok(())
    }

    fn kbl_cmd(&mut self, args: Option<&str>) -> anyhow::Result<()> {
        let name = args.unwrap_or("us");
        if set_layout(name).is_none() {
            _ = writeln!(self.term, "invalid layout `{name}`");
            _ = writeln!(self.term, "available layouts(s): `{:?}`", layouts());
        }

        Ok(())
    }

    fn help_cmd(&mut self, _: Option<&str>) -> anyhow::Result<()> {
        _ = writeln!(
            self.term,
            "available built-in shell commands:\nkbl, help, kill, exit, clear, lspci"
        );

        Ok(())
    }

    fn kill_cmd(&mut self, args: Option<&str>) -> anyhow::Result<()> {
        let Some(arg) = args else {
            return Err(anyhow!("missing arg pid"));
        };

        let Ok(pid) = arg.parse::<usize>() else {
            return Err(anyhow!("invalid arg pid"));
        };

        let Some(proc) = Pid::new(pid).find() else {
            return Err(anyhow!("couldn't find the process"));
        };

        proc.should_terminate.store(true, Ordering::SeqCst);

        Ok(())
    }

    fn lspci_cmd(&mut self, _args: Option<&str>) -> anyhow::Result<()> {
        for device in hyperion_pci::devices() {
            _ = writeln!(self.term, "{device}");
        }

        Ok(())
    }
}
