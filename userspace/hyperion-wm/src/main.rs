#![feature(slice_as_chunks, lazy_cell)]

//

#[cfg(not(feature = "cargo-clippy"))]
use std::os::hyperion::{
    net::{LocalListener, LocalStream},
    AsRawFd,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{stderr, stdout, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    ptr::{self, NonNull},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, LazyLock, Mutex,
    },
    thread,
};

use hyperion_syscall::{
    fs::{FileDesc, FileOpenFlags},
    get_tid, map_file, nanosleep_until, system, timestamp, unmap_file,
};

//

// clippy doesn't support x86_64-unknown-hyperion
#[cfg(feature = "cargo-clippy")]
struct LocalListener;

#[cfg(feature = "cargo-clippy")]
impl LocalListener {
    pub fn bind(_: &str) -> Result<Self, ()> {
        todo!()
    }

    pub fn accept(&self) -> Result<LocalStream, ()> {
        todo!()
    }
}

// clippy doesn't support x86_64-unknown-hyperion
#[cfg(feature = "cargo-clippy")]
struct LocalStream;

#[cfg(feature = "cargo-clippy")]
impl LocalStream {
    // pub fn connect(_: &str) -> Result<Self, ()> {
    //     todo!()
    // }
}

#[cfg(feature = "cargo-clippy")]
impl std::io::Read for LocalStream {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        todo!()
    }
}

#[cfg(feature = "cargo-clippy")]
impl BufRead for LocalStream {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        todo!()
    }

    fn consume(&mut self, _: usize) {}
}

#[cfg(feature = "cargo-clippy")]
impl Write for LocalStream {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        todo!()
    }

    fn flush(&mut self) -> std::io::Result<()> {
        todo!()
    }
}

#[cfg(feature = "cargo-clippy")]
trait AsRawFd {
    fn as_raw_fd(&self) -> usize;
}

#[cfg(feature = "cargo-clippy")]
impl AsRawFd for File {
    fn as_raw_fd(&self) -> usize {
        todo!()
    }
}

//

fn framebuffer_info() -> Framebuffer {
    let fbo_info = OpenOptions::new().read(true).open("/dev/fb0-info").unwrap();
    let fbo_info = BufReader::new(fbo_info);

    let line = fbo_info.lines().next().unwrap().unwrap();

    let mut fbo_info_iter = line.split(':');
    let width = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();
    let height = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();
    let pitch = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();
    // let bpp = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();

    Framebuffer {
        width,
        height,
        pitch,
    }
}

#[derive(Debug)]
struct Framebuffer {
    width: usize,
    height: usize,
    pitch: usize,
}

struct Window {
    info: WindowInfo,
    shmem: Option<File>,
    shmem_ptr: Option<NonNull<()>>,

    events: Arc<LocalStream>,
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

    fs::create_dir_all("/run").unwrap();

    let server = LocalListener::bind("/run/wm.socket").unwrap();

    system("/bin/term", &[]).unwrap();
    system("/bin/term", &[]).unwrap();
    system("/bin/term", &[]).unwrap();

    loop {
        let client: LocalStream = server.accept().unwrap();

        thread::spawn(move || handle_client(client));
    }
}

fn keyboard() {
    let windows = &*WINDOWS;

    let mut keyboard = File::open("/dev/keyboard").unwrap();

    let mut buf = [0u8; 8];

    loop {
        let _n = keyboard.read(&mut buf).unwrap();

        let _windows = windows.lock().unwrap();
        if let Some(window) = _windows.get(0) {
            writeln!(&mut &*window.events, "event keyboard").unwrap();
        }
        drop(_windows);
    }
}

fn blitter() {
    let windows = &*WINDOWS;

    println!("display = tid:{}", get_tid());

    // stdio_to_logfile();
    let mut global_fb = lock_global_fb();
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

// erease the type information for rust-analyzer
// (because rust-analyzer doesn't support x86_64-unknown-hyperion)
fn handle_client(client: LocalStream) {
    let windows = &*WINDOWS;

    let client = Arc::new(client);
    let mut client_send = client.clone();
    let mut client_recv = BufReader::new(&*client);

    // a super simple display protocol
    let mut buf = String::new();

    loop {
        buf.clear();
        let len = client_recv.read_line(&mut buf).unwrap();
        let line = buf[..len].trim();

        if line.is_empty() {
            continue;
        }

        match line {
            "new_window" => {
                println!("client requested a new window");

                static X: AtomicUsize = AtomicUsize::new(10);
                static Y: AtomicUsize = AtomicUsize::new(10);

                let x = X.fetch_add(210, Ordering::Relaxed);
                let y = Y.load(Ordering::Relaxed);

                let mut _windows = windows.lock().unwrap();
                let window_id = _windows.len();
                _windows.push(Window {
                    shmem: None,
                    shmem_ptr: None,
                    info: WindowInfo {
                        id: window_id,
                        x,
                        y,
                        w: 200,
                        h: 200,
                    },
                    events: client_send.clone(),
                });
                drop(_windows);

                // TODO: anonymous file + pass the fd instead of making a file that any proc can read
                let path = format!("/run/wm.window.{window_id}");
                // TODO: create_new
                let mut window_file = File::create(path.as_str()).unwrap();
                // TODO: truncate
                window_file
                    .seek(SeekFrom::Start(200 * 200 * 4 - 4))
                    .unwrap();
                window_file.write_all(&[0u8; 4]).unwrap();
                let len = window_file.metadata().unwrap().len() as usize;

                let shmem_ptr: NonNull<()> =
                    map_file(FileDesc(window_file.as_raw_fd()), None, len, 0).unwrap();
                println!("shmem_ptr={:#018x}", shmem_ptr.as_ptr() as usize);
                let shmem = ptr::slice_from_raw_parts_mut(shmem_ptr.as_ptr().cast::<u8>(), len);
                let shmem = unsafe { &mut *shmem };
                shmem.fill(0);

                let mut _windows = windows.lock().unwrap();
                let window = &mut _windows[window_id];
                window.shmem = Some(window_file);
                window.shmem_ptr = Some(shmem_ptr);
                drop(_windows);

                buf.clear();
                use std::fmt::Write;
                writeln!(buf, "new_window {window_id}").unwrap();
                client_send.write_all(buf.as_bytes()).unwrap();
            }
            _ => {
                println!("unknown command `{line}`")
            }
        }

        println!("request handled");
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

#[allow(unused)]
fn lock_global_fb() -> GlobalFb {
    let mut info = framebuffer_info();

    println!("fb0 = {info:?}");

    let fbo = OpenOptions::new()
        .write(true)
        .open("/dev/fb0")
        .expect("failed to open /dev/fb0");
    let meta = fbo.metadata().expect("failed to read fb file metadata");

    let fbo_fd = FileDesc(AsRawFd::as_raw_fd(&fbo) as _);

    let fbo_mapped: NonNull<()> =
        map_file(fbo_fd, None, meta.len() as _, 0).expect("failed to map the fb");

    let buf = ptr::slice_from_raw_parts_mut(fbo_mapped.as_ptr().cast(), meta.len() as _);
    // let mut backbuf = vec![0u8; buf.len()];
    // info.buf = &mut backbuf;

    GlobalFb {
        width: info.width,
        height: info.height,
        pitch: info.pitch,

        buf,
        fbo,
        fbo_fd,
        fbo_mapped,
    }
}

#[allow(unused)]
struct GlobalFb {
    width: usize,
    height: usize,
    pitch: usize,

    buf: *mut [u8],
    // backbuf: Box<&mut [u8]>,
    fbo: File,
    fbo_fd: FileDesc,
    fbo_mapped: NonNull<()>,
}

impl GlobalFb {
    #[allow(unused)]
    fn buf_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *self.buf }
    }
}

impl Drop for GlobalFb {
    fn drop(&mut self) {
        unmap_file(self.fbo_fd, self.fbo_mapped, 0).expect("failed to unmap the fb");
    }
}
