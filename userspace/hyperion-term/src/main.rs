#[cfg(not(feature = "cargo-clippy"))]
use std::os::hyperion::{net::LocalStream, AsRawFd};
use std::{
    fs::File,
    io::{self, BufRead, Write},
    ptr::NonNull,
    sync::{Arc, Mutex, MutexGuard},
};

use hyperion_color::Color;
use hyperion_syscall::{fs::FileDesc, get_pid, map_file, nanosleep_until, timestamp};

//

// clippy doesn't support x86_64-unknown-hyperion
#[cfg(feature = "cargo-clippy")]
struct LocalStream;

#[cfg(feature = "cargo-clippy")]
impl LocalStream {
    pub fn connect(_: &str) -> std::io::Result<Self> {
        todo!()
    }
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

fn main() {
    let wm = Connection::new().unwrap();

    let mut window = wm.new_window().unwrap();

    let colors = [Color::RED, Color::GREEN, Color::BLUE];
    let i = get_pid();

    let mut t = timestamp().unwrap() as u64;
    loop {
        window.fill(colors[i % 3]);

        t += 16_666_667;
        nanosleep_until(t);
    }
}

#[allow(unused)]
pub struct Window {
    conn: Connection,

    fbo: File,
    fbo_ptr: NonNull<()>, // TODO: volatile write
    width: usize,
    height: usize,
    pitch: usize,
}

impl Window {
    pub fn fill(&mut self, color: Color) {
        let color = color.as_u32();
        let pixels = self.fbo_ptr.cast::<u32>().as_ptr();

        for y in 0..self.height {
            for x in 0..self.width {
                // Rust should vectorize this
                // fill doesn't work because this memory is volatile
                unsafe { pixels.add(x + y * self.pitch).write_volatile(color) };
            }
        }
    }
}

#[derive(Clone)]
pub struct Connection {
    inner: Arc<Mutex<ConnectionInner>>,
}

struct ConnectionInner {
    socket: LocalStream,
    buf: String,
}

impl Connection {
    pub fn new() -> io::Result<Self> {
        let socket = LocalStream::connect("/run/wm.socket")?;
        Ok(Self {
            inner: Arc::new(Mutex::new(ConnectionInner {
                socket,
                buf: String::new(),
            })),
        })
    }

    pub fn cmd(&self, str: &str) -> io::Result<String> {
        let mut inner = self.inner.lock().unwrap();
        inner.cmd(str).map(ToString::to_string)
    }

    pub fn new_window(&self) -> io::Result<Window> {
        let mut inner = self.inner();
        println!("requesting window");
        let result = inner.cmd("new_window")?;

        let window_id = result.parse::<usize>().unwrap();
        println!("got window_id={window_id}");

        let path = format!("/run/wm.window.{window_id}");
        let fbo = File::options()
            .read(true)
            .write(true)
            .create(false)
            .open(path)
            .unwrap();
        let size = fbo.metadata().unwrap().len() as usize;

        let fbo_ptr = map_file(FileDesc(fbo.as_raw_fd()), None, size, 0).unwrap();

        Ok(Window {
            conn: self.clone(),
            fbo,
            fbo_ptr,
            width: 200,
            height: 200,
            pitch: 200,
        })
    }

    fn inner(&self) -> MutexGuard<ConnectionInner> {
        self.inner.lock().unwrap()
    }
}

impl ConnectionInner {
    pub fn cmd(&mut self, str: &str) -> io::Result<&str> {
        // send the command
        self.socket.write_all(str.as_bytes())?;
        self.socket.write_all(b"\n")?;

        // wait for the result
        self.buf.clear();
        let len = self.socket.read_line(&mut self.buf)?;
        let result = self.buf[..len].trim();

        Ok(result)
    }
}
