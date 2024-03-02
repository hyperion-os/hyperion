#![feature(slice_as_chunks, lazy_cell)]

//

use std::{
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashSet},
    fs::File,
    io::{stderr, stdout, Read},
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
    pub closed: bool,

    conn: MessageStream,
    shmem: File,
    shmem_ptr: NonNull<u32>,
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

static WINDOWS: Mutex<
    Vec<Window>, // Z sorted window ids, furthest is first
> = Mutex::new(Vec::new());
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

    loop {
        let client = server.accept().unwrap();
        thread::spawn(move || handle_client(client));
    }
}

fn event_handler() {
    let mut cursor = (0.0, 0.0);
    let mut alt_held = false; // alt instead of super, because I run this in QEMU and I CBA to capture keyboard
    let mut dragging: Option<(f32, f32)> = None;

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

                if let Some(init) = dragging {
                    let mut windows = WINDOWS.lock().unwrap();
                    if let Some(window) = windows.last_mut() {
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
                let mut l = WINDOWS.lock().unwrap();

                if let Some((hit_idx, _)) = l.iter().enumerate().rfind(|(_, window)| {
                    window.info.x <= cursor.0 as usize
                        && cursor.0 as usize <= window.info.x + window.info.w
                        && window.info.y <= cursor.1 as usize
                        && cursor.1 as usize <= window.info.y + window.info.h
                }) {
                    // println!("clicked on {hit_idx}");
                    let active = l.remove(hit_idx);
                    // mark the held window
                    if alt_held {
                        dragging = Some((
                            active.info.x as f32 - cursor.0,
                            active.info.y as f32 - cursor.1,
                        ));
                    }
                    // set the window as active
                    l.push(active);
                }
            }
            Event::Keyboard {
                code: 72,
                state: ElementState::Pressed,
            } if alt_held => {
                system("/bin/term", &[]).unwrap();
            }
            _ if alt_held => {}
            ev => {
                let windows = WINDOWS.lock().unwrap();
                if let Some(active) = windows.last() {
                    // println!("sending {ev:?}");
                    _ = active.conn.send_message(Message::Event(ev));
                }
            }
        }
    }
}

fn handle_client(client: Connection) {
    let mut rand_dev = File::open("/dev/random").unwrap();
    let mut rng = || {
        let mut b = [0u8; 8];
        rand_dev.read_exact(&mut b).unwrap();
        usize::from_ne_bytes(b)
    };

    let monitor_size = (600, 600); // FIXME:

    let mut own_windows = HashSet::new();

    while let Ok(ev) = client.next_request() {
        match ev {
            Request::NewWindow => {
                println!("client requested a new window");

                static ID: AtomicUsize = AtomicUsize::new(0);

                let id = ID.fetch_add(1, Ordering::Relaxed);
                let x = rng() % monitor_size.0;
                let y = rng() % monitor_size.1;

                let (window_file, shmem_ptr) = new_window_framebuffer(400, 400, id);

                let mut windows = WINDOWS.lock().unwrap();
                windows.push(Window {
                    info: WindowInfo {
                        id,
                        x,
                        y,
                        w: 400,
                        h: 400,
                    },
                    old_info: WindowInfo::default(),
                    closed: false,
                    conn: client.clone_tx(),
                    shmem: window_file,
                    shmem_ptr,
                });
                drop(windows);
                own_windows.insert(id);

                if client
                    .send_message(Message::NewWindow { window_id: id })
                    .is_err()
                {
                    break;
                }
            }
            Request::CloseConnection => {
                let mut windows = WINDOWS.lock().unwrap();
                for window in windows.iter_mut() {
                    if own_windows.contains(&window.info.id) {
                        window.info = <_>::default();
                        window.closed = true;
                    }
                }
                break;
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
