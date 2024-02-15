#![feature(slice_as_chunks, lazy_cell, core_intrinsics)]
#![allow(internal_features)]

//

use std::{
    fs::File,
    intrinsics::{volatile_copy_nonoverlapping_memory, volatile_store},
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
    get_tid, system, timestamp,
};
use hyperion_windowing::{
    global::GlobalFb,
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

    thread::spawn(blitter);
    thread::spawn(keyboard);
    thread::spawn(mouse::mouse);

    let server = Server::new().unwrap();

    system("/bin/term", &[]).unwrap();
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

    println!("display = tid:{}", get_tid());

    // stdio_to_logfile();
    let global_fb = GlobalFb::lock_global_fb();
    let width = global_fb.width;
    let height = global_fb.height;
    let pitch = global_fb.pitch / 4;

    let mut global_fb = Region {
        buf: global_fb.buf.cast(),
        pitch,
        width,
        height,
    };

    global_fb.volatile_fill(0, 0, width, height, Color::from_hex("#141414").unwrap());

    let mut next_sync = timestamp().unwrap() as u64;
    loop {
        let _windows = windows.lock().unwrap();
        // println!("windows={}", _windows.len());
        for (info, pixels) in _windows.iter().filter_map(|w| Some((w.info, w.shmem_ptr?))) {
            let window = Region {
                buf: pixels.as_ptr(),
                pitch: info.w,
                width: info.w,
                height: info.h,
            };

            // TODO: smarter blitting to avoid copying every single window every single frame
            global_fb.volatile_copy_from(&window, info.x as isize, info.y as isize);
        }
        drop(_windows);

        let (m_x, m_y) = get_mouse();
        let (c_x, c_y) = (m_x as usize, m_y as usize);

        global_fb.volatile_fill(c_x, c_y, 16, 16, Color::WHITE);

        // println!("VSYNC");
        next_sync += 16_666_667;
        hyperion_syscall::nanosleep_until(next_sync);

        // hyperion_syscall::yield_now();

        global_fb.volatile_fill(c_x, c_y, 16, 16, Color::from_hex("#141414").unwrap());
    }
}

#[derive(Debug, Clone, Copy)]
struct Region {
    buf: *mut u32,
    pitch: usize,  // offset to the next line aka. the real width
    width: usize,  // width of the region
    height: usize, // height of the region
}

impl Region {
    // fn sub_region(self, x: usize, y: usize, width: usize, height: usize) -> Option<Region> {
    //     if x >= self.pitch {
    //         panic!();
    //     }
    //     if y >= self.height {
    //         panic!();
    //     }
    // }

    fn volatile_copy_from(&mut self, from: &Region, to_x: isize, to_y: isize) {
        // https://gdbooks.gitbooks.io/3dcollisions/content/Chapter2/static_aabb_aabb.html

        // let xmin_a = 0usize; // left
        // let ymin_a = 0usize; // up
        let xmax_a = self.width; // right
        let ymax_a = self.height; // down

        let xmin_b = to_x.max(0) as usize; // left
        let ymin_b = to_y.max(0) as usize; // up
        let xmax_b = (to_x + from.width as isize).max(0) as usize; // right
        let ymax_b = (to_y + from.height as isize).max(0) as usize; // down

        let xmin = xmin_b;
        let ymin = ymin_b;
        let xmax = xmax_a.min(xmax_b);
        let ymax = ymax_a.min(ymax_b);

        let x = xmin;
        let x_len = xmax - x;
        let y = ymin;
        let y_len = ymax - y;

        if x_len <= 0 || y_len <= 0 {
            return;
        }

        assert!(xmax <= self.width);
        assert!(xmax <= self.height);

        assert!(x as isize - to_x >= 0);
        assert!(y as isize - to_y >= 0);
        assert!(xmax.checked_add_signed(-to_x).unwrap() <= from.width);
        assert!(ymax.checked_add_signed(-to_y).unwrap() <= from.height);

        for y in ymin..ymax {
            let to_spot = x + y * self.pitch;
            let from_spot =
                x.wrapping_add_signed(-to_x) + y.wrapping_add_signed(-to_y) * from.pitch;

            let to = unsafe { self.buf.add(to_spot) };
            let from = unsafe { from.buf.add(from_spot) };

            unsafe {
                volatile_copy_nonoverlapping_memory(to, from, x_len);
            }
        }
    }

    fn volatile_fill(&mut self, x: usize, y: usize, w: usize, h: usize, col: Color) {
        let x_len = self.width.min(x + w).saturating_sub(x);
        let y_len = self.height.min(y + h).saturating_sub(y);

        // println!("x={x} y={y} w={w} h={h} x_len={x_len} y_len={y_len}");

        if x_len <= 0 || y_len <= 0 {
            return;
        }

        let col = col.as_u32();

        for y in y..y + y_len {
            for x in x..x + x_len {
                let to_spot = x + y * self.pitch;
                let to = unsafe { self.buf.add(to_spot) };

                unsafe { volatile_store(to, col) }
            }
        }
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

                let x = X.fetch_add(210, Ordering::Relaxed);
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
                        w: 200,
                        h: 200,
                    },
                });
                drop(_windows);

                let (window_file, shmem_ptr) = new_window_framebuffer(200, 200, window_id);

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
