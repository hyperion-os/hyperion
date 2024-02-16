#![feature(slice_as_chunks, lazy_cell)]

//

use std::{
    fs::File,
    io::{stderr, stdout, Read},
    ptr::NonNull,
    sync::{
        atomic::{AtomicUsize, Ordering},
        LazyLock, Mutex,
    },
    thread,
};

use hyperion_color::Color;
use hyperion_syscall::{
    fs::{FileDesc, FileOpenFlags},
    system, timestamp,
};
use hyperion_windowing::{
    global::{GlobalFb, Region},
    server::{new_window_framebuffer, Connection, MessageStream, Server},
    shared::{ElementState, Event, Message, Request},
};
use pc_keyboard::{
    layouts::{AnyLayout, Us104Key},
    DecodedKey, HandleControl, KeyState, Keyboard, ScancodeSet1,
};

use crate::mouse::get_mouse;

//

mod mouse;

//

pub struct Window {
    conn: MessageStream,
    pub info: WindowInfo,
    shmem: Option<File>,
    shmem_ptr: Option<NonNull<u32>>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WindowInfo {
    pub id: usize,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

unsafe impl Sync for Window {}
unsafe impl Send for Window {}

static WINDOWS: LazyLock<Mutex<Vec<Window>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

//

fn main() {
    stdio_to_logfile();

    let server = Server::new().unwrap();

    thread::spawn(blitter);
    thread::spawn(keyboard);
    thread::spawn(mouse::mouse);

    system("/bin/term", &[]).unwrap();
    system("/bin/term", &[]).unwrap();

    loop {
        let client = server.accept().unwrap();
        thread::spawn(move || handle_client(client));
    }
}

fn keyboard() {
    let windows = &*WINDOWS;

    let mut kb_dev = File::open("/dev/keyboard").unwrap();

    let mut buf = [0u8; 64];

    let mut keyboard = Keyboard::new(
        ScancodeSet1::new(),
        AnyLayout::Us104Key(Us104Key),
        HandleControl::Ignore,
    );

    loop {
        let n = kb_dev.read(&mut buf).unwrap();

        // let windows = LazyCell::new(|| windows.lock().unwrap());
        // let windows = LazyCell::new(|| windows.last());

        let windows = windows.lock().unwrap();
        if let Some(window) = windows.get(ACTIVE.load(Ordering::Relaxed)) {
            for byte in &buf[..n] {
                if let Ok(Some(ev)) = keyboard.add_byte(*byte) {
                    let code = ev.code as u8;
                    if ev.state != KeyState::Up {
                        // down or single shot
                        window.conn.send_message(Message::Event(Event::Keyboard {
                            code,
                            state: ElementState::Pressed,
                        }));
                    }
                    if ev.state != KeyState::Down {
                        // this is intentionally not an `else if`, single shot presses send both
                        // up or single shot
                        window.conn.send_message(Message::Event(Event::Keyboard {
                            code,
                            state: ElementState::Released,
                        }));
                    }
                    if let Some(DecodedKey::Unicode(ch)) = keyboard.process_keyevent(ev) {
                        window.conn.send_message(Message::Event(Event::Text { ch }));
                    }
                }
            }
        }
        drop(windows);
    }
}

fn blitter() {
    let windows = &*WINDOWS;

    let mut global_fb = GlobalFb::lock_global_fb();
    let mut global_fb = global_fb.as_region();

    // background
    global_fb.volatile_fill(
        0,
        0,
        usize::MAX,
        usize::MAX,
        Color::from_hex("#141414").unwrap().as_u32(),
    );

    let mut next_sync = timestamp().unwrap() as u64;
    loop {
        // blit all windows
        let _windows = windows.lock().unwrap();
        for (info, pixels) in _windows.iter().filter_map(|w| Some((w.info, w.shmem_ptr?))) {
            let window = unsafe { Region::new(pixels.as_ptr(), info.w, info.w, info.h) };

            // TODO: smarter blitting to avoid copying every single window every single frame
            global_fb.volatile_copy_from(&window, info.x as isize, info.y as isize);
        }
        drop(_windows);

        // blit cursor
        let (m_x, m_y) = get_mouse();
        let (c_x, c_y) = (m_x as usize, m_y as usize);
        global_fb.volatile_fill(c_x, c_y, 16, 16, Color::WHITE.as_u32());

        // println!("VSYNC");
        next_sync += 16_666_667;
        hyperion_syscall::nanosleep_until(next_sync);

        // hyperion_syscall::yield_now();

        // remove the old cursor
        global_fb.volatile_fill(
            c_x,
            c_y,
            16,
            16,
            Color::from_hex("#141414").unwrap().as_u32(),
        );
    }
}

fn handle_client(client: Connection) {
    let windows = &*WINDOWS;

    loop {
        match client.next_request() {
            Request::NewWindow => {
                println!("client requested a new window");

                static X: AtomicUsize = AtomicUsize::new(10);
                static Y: AtomicUsize = AtomicUsize::new(10);

                let x = X.fetch_add(410, Ordering::Relaxed);
                let y = Y.load(Ordering::Relaxed);

                let mut _windows = windows.lock().unwrap();
                let window_id = _windows.len();
                _windows.push(Window {
                    conn: client.clone_tx(),
                    shmem: None,
                    shmem_ptr: None,
                    info: WindowInfo {
                        id: window_id,
                        x,
                        y,
                        w: 400,
                        h: 400,
                    },
                });
                drop(_windows);

                let (window_file, shmem_ptr) = new_window_framebuffer(400, 400, window_id);

                let mut _windows = windows.lock().unwrap();
                let window = &mut _windows[window_id];
                window.shmem = Some(window_file);
                window.shmem_ptr = Some(shmem_ptr);
                drop(_windows);

                client.send_message(Message::NewWindow { window_id });
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
