#![feature(slice_as_chunks, lazy_cell)]

//

use std::{
    fs::File,
    io::{stderr, stdout},
    mem,
    ptr::NonNull,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        LazyLock, Mutex,
    },
    thread,
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use hyperion_syscall::{
    fs::{FileDesc, FileOpenFlags},
    system,
};
use hyperion_windowing::{
    server::{new_window_framebuffer, Connection, MessageStream, Server},
    shared::{Button, ElementState, Event, Message, Mouse, Request},
};

//

mod blit;
mod keyboard;
mod mouse;

//

pub struct Window {
    pub info: WindowInfo,
    // pre-update cache for the blitter
    pub old_info: WindowInfo,

    conn: MessageStream,
    shmem: Option<File>,
    shmem_ptr: Option<NonNull<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowInfo {
    pub id: usize,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

unsafe impl Sync for Window {}
unsafe impl Send for Window {}

//

pub struct AtomicCursor {
    inner: AtomicU64,
}

impl AtomicCursor {
    pub const fn new(pos: (f32, f32)) -> Self {
        Self {
            inner: AtomicU64::new(Self::pack(pos)),
        }
    }

    pub fn load(&self) -> (f32, f32) {
        Self::unpack(self.inner.load(Ordering::Acquire))
    }

    pub fn store(&self, pos: (f32, f32)) {
        self.inner.store(Self::pack(pos), Ordering::Release);
    }

    const fn pack(pos: (f32, f32)) -> u64 {
        unsafe { mem::transmute(pos) }
    }

    const fn unpack(packed: u64) -> (f32, f32) {
        unsafe { mem::transmute(packed) }
    }
}

//

static WINDOWS: LazyLock<Mutex<Vec<Window>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static EVENTS: LazyLock<(Sender<Event>, Receiver<Event>)> = LazyLock::new(unbounded);
static CURSOR: AtomicCursor = AtomicCursor::new((0.0, 0.0));

//

fn main() {
    stdio_to_logfile();

    let server = Server::new().unwrap();

    thread::spawn(blit::blitter);
    thread::spawn(keyboard::keyboard);
    thread::spawn(mouse::mouse);
    thread::spawn(event_handler);

    system("/bin/term", &[]).unwrap();
    system("/bin/term", &[]).unwrap();

    loop {
        let client = server.accept().unwrap();
        thread::spawn(move || handle_client(client));
    }
}

fn event_handler() {
    let mut cursor = (0.0, 0.0);
    let mut alt_held = false; // alt instead of super, because I run this in QEMU and I CBA to capture keyboard
    let mut dragging: Option<((f32, f32), usize)> = None;

    while let Ok(ev) = EVENTS.1.recv() {
        // println!("handle ev: {ev:?}");

        match ev {
            Event::Keyboard { code: 95, state } => {
                alt_held = state == ElementState::Pressed;
            }
            Event::Mouse(Mouse::Motion { x, y }) => {
                cursor.0 = (cursor.0 + x).max(0.0);
                cursor.1 = (cursor.1 - y).max(0.0);
                CURSOR.store(cursor);

                if let Some((init, w_id)) = dragging {
                    let mut windows = WINDOWS.lock().unwrap();
                    if let Some(window @ Window { .. }) = windows.get_mut(w_id) {
                        // FIXME: the Rust typechecker blows up without @ Window { .. } for some reason
                        window.info.x = (init.0 + cursor.0) as usize;
                        window.info.y = (init.1 + cursor.1) as usize;
                    }
                }
            }
            Event::Mouse(Mouse::Button {
                btn: Button::Left,
                state: ElementState::Released,
            }) => dragging = None,
            Event::Mouse(Mouse::Button {
                btn: Button::Left,
                state: ElementState::Pressed,
            }) => {
                let windows = WINDOWS.lock().unwrap();
                for (i, window) in windows.iter().enumerate() {
                    if window.info.x <= cursor.0 as usize
                        && cursor.0 as usize <= window.info.x + window.info.w
                        && window.info.y <= cursor.1 as usize
                        && cursor.1 as usize <= window.info.y + window.info.h
                    {
                        println!("switch active window to {i}");
                        ACTIVE.store(i, Ordering::Relaxed);

                        if alt_held {
                            dragging = Some((
                                (
                                    window.info.x as f32 - cursor.0,
                                    window.info.y as f32 - cursor.1,
                                ),
                                i,
                            ));
                        }
                    }
                }
            }
            ev => {
                let windows = WINDOWS.lock().unwrap();
                if let Some(active_window) = windows.get(ACTIVE.load(Ordering::Relaxed)) {
                    println!("sending {ev:?}");
                    active_window.conn.send_message(Message::Event(ev)).unwrap();
                }
            }
        }
    }
}

fn handle_client(client: Connection) {
    let windows = &*WINDOWS;

    while let Ok(ev) = client.next_request() {
        match ev {
            Request::NewWindow => {
                println!("client requested a new window");

                static X: AtomicUsize = AtomicUsize::new(10);
                static Y: AtomicUsize = AtomicUsize::new(10);

                let x = X.fetch_add(410, Ordering::Relaxed);
                let y = Y.load(Ordering::Relaxed);

                let mut _windows = windows.lock().unwrap();
                let window_id = _windows.len();
                _windows.push(Window {
                    info: WindowInfo {
                        id: window_id,
                        x,
                        y,
                        w: 400,
                        h: 400,
                    },
                    old_info: WindowInfo::default(),
                    conn: client.clone_tx(),
                    shmem: None,
                    shmem_ptr: None,
                });
                drop(_windows);

                let (window_file, shmem_ptr) = new_window_framebuffer(400, 400, window_id);

                let mut _windows = windows.lock().unwrap();
                let window = &mut _windows[window_id];
                window.shmem = Some(window_file);
                window.shmem_ptr = Some(shmem_ptr);
                drop(_windows);

                if client
                    .send_message(Message::NewWindow { window_id })
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}

#[allow(unused)]
fn stdio_to_logfile() {
    // other threads shouldn't use stdio while this happens
    let stdout = stdout().lock();
    let stderr = stderr().lock();

    // close stdout and stderr, replace them with a log file
    hyperion_syscall::close(FileDesc(1)).unwrap();
    // hyperion_syscall::open(
    //     "/tmp/wm.stdout.log",
    //     FileOpenFlags::CREATE | FileOpenFlags::WRITE | FileOpenFlags::TRUNC,
    //     0,
    // )
    // .unwrap();
    hyperion_syscall::open("/dev/log", FileOpenFlags::WRITE, 0).unwrap();
    hyperion_syscall::close(FileDesc(2)).unwrap();
    // hyperion_syscall::open(
    //     "/tmp/wm.stderr.log",
    //     FileOpenFlags::CREATE | FileOpenFlags::WRITE | FileOpenFlags::TRUNC,
    //     0,
    // )
    // .unwrap();
    hyperion_syscall::open("/dev/log", FileOpenFlags::WRITE, 0).unwrap();

    drop((stdout, stderr))
}
