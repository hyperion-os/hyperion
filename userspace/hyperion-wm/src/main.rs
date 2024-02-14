#![feature(slice_as_chunks, lazy_cell)]

//

use std::{
    fs::File,
    io::{stderr, stdout, Read},
    ptr::{self, NonNull},
    sync::{
        atomic::{AtomicUsize, Ordering},
        LazyLock, Mutex,
    },
    thread,
};

use hyperion_syscall::{
    fs::{FileDesc, FileOpenFlags},
    get_tid, nanosleep_until, system, timestamp,
};
use hyperion_windowing::{
    global::GlobalFb,
    server::{new_window_framebuffer, Connection, MessageStream, Server},
    shared::{Event, Message, Request},
};
use pc_keyboard::{
    layouts::{AnyLayout, Us104Key},
    DecodedKey, HandleControl, KeyState, Keyboard, ScancodeSet1,
};

//

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

fn main() {
    stdio_to_logfile();

    thread::spawn(blitter);
    thread::spawn(keyboard);

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

    let mut buf = [0u8; 8];

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
        if let Some(window) = windows.get(0) {
            for byte in &buf[..n] {
                if let Ok(Some(ev)) = keyboard.add_byte(*byte) {
                    let code = ev.code as u8;
                    if ev.state != KeyState::Up {
                        // down or single shot
                        window
                            .conn
                            .send_message(Message::Event(Event::Keyboard { code, state: 1 }));
                    }
                    if ev.state != KeyState::Down {
                        // this is intentionally not an `else if`, single shot presses send both
                        // up or single shot
                        window
                            .conn
                            .send_message(Message::Event(Event::Keyboard { code, state: 0 }));
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
    let mut global_fb = GlobalFb::lock_global_fb();
    let width = global_fb.width;
    let height = global_fb.height;
    let pitch = global_fb.pitch;

    global_fb.buf_mut().fill(20);

    let mut next_sync = timestamp().unwrap() as u64;
    loop {
        let _windows = windows.lock().unwrap();
        // println!("windows={}", _windows.len());
        for (info, pixels) in _windows.iter().filter_map(|w| Some((w.info, w.shmem_ptr?))) {
            let window_pixels =
                ptr::slice_from_raw_parts(pixels.as_ptr().cast_const().cast(), info.w * info.h * 4);
            let window_pixels = unsafe { &*window_pixels };

            let global_pixels = global_fb.buf_mut();

            // println!("VSYNC");
            // TODO: smarter blitting to avoid copying every single window every single frame
            for y in 0..info.h {
                let gy = y + info.y;

                if gy >= height {
                    break;
                }

                let from_line: &[u8] = &window_pixels[y * info.w * 4..(y + 1) * info.w * 4];
                let to_line: &mut [u8] = &mut global_pixels[gy * pitch..gy * pitch + width * 4];

                // println!(
                //     "from_line=[.., {}] info.w=[.., {}]",
                //     from_line.len(),
                //     to_line.len()
                // );

                // println!(
                //     "from_line={:#018x} info.w={:#018x}",
                //     from_line.as_ptr() as usize,
                //     to_line.as_ptr() as usize,
                // );

                let to_line = &mut to_line[info.x * 4..];

                let limit = from_line.len().min(to_line.len());

                // FIXME: windows arent always at (0,0) and smaller than the global fb
                let from_line = &from_line[..limit];
                let to_line = &mut to_line[..limit];

                to_line.copy_from_slice(from_line);

                // to_line.fill(0);
            }
        }
        drop(_windows);

        // println!("VSYNC");
        next_sync += 16_666_667;
        nanosleep_until(next_sync);
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
