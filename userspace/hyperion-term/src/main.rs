#[cfg(not(feature = "cargo-clippy"))]
use std::os::hyperion::{net::LocalStream, AsRawFd};
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Write},
    ptr::NonNull,
    sync::{Arc, Mutex},
};

use hyperion_color::Color;
use hyperion_syscall::{fs::FileDesc, get_pid, map_file, nanosleep_until, timestamp};

//

// clippy doesn't support x86_64-unknown-hyperion
#[cfg(any(feature = "cargo-clippy", feature = "rust-analyzer"))]
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

pub enum Event {
    NewWindow { id: usize },
}

#[derive(Clone)]
pub struct Connection {
    inner: Arc<ConnectionInner>,
}

struct ConnectionInner {
    read: Mutex<ConnectionRead>,
    write: ConnectionWrite,
}

struct ConnectionRead {
    socket_r: BufReader<Arc<LocalStream>>,
    buf: String,
    event_buf: Vec<Event>,
}

struct ConnectionWrite {
    socket_w: Arc<LocalStream>,
}

impl Connection {
    pub fn new() -> io::Result<Self> {
        let socket = Arc::new(LocalStream::connect("/run/wm.socket")?);
        let socket_r = BufReader::new(socket.clone());
        let socket_w = socket;

        Ok(Self {
            inner: Arc::new(ConnectionInner {
                read: Mutex::new(ConnectionRead {
                    socket_r,
                    buf: String::new(),
                    event_buf: Vec::new(),
                }),
                write: ConnectionWrite { socket_w },
            }),
        })
    }

    pub fn new_window(&self) -> io::Result<Window> {
        println!("requesting window");
        self.inner.cmd("new_window")?;

        println!("waiting for new_window");
        let ev = self.inner.next_event(Some("new_window"))?;

        let Event::NewWindow { id: window_id } = ev;
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
}

impl ConnectionInner {
    pub fn cmd(&self, str: &str) -> io::Result<()> {
        self.write.cmd(str)
    }

    pub fn next_event(&self, filter: Option<&str>) -> io::Result<Event> {
        self.read.lock().unwrap().next_event(filter)
    }
}

impl ConnectionWrite {
    /// the result of the command will come back at some point into the event buffer
    pub fn cmd(&self, str: &str) -> io::Result<()> {
        // send the command
        (&mut self.socket_w.as_ref()).write_all(str.as_bytes())?;
        (&mut self.socket_w.as_ref()).write_all(b"\n")?;

        Ok(())
    }
}

impl ConnectionRead {
    pub fn next_event(&mut self, filter: Option<&str>) -> io::Result<Event> {
        loop {
            // wait for the result
            self.buf.clear();
            let len = self.socket_r.read_line(&mut self.buf)?;
            let result = self.buf[..len].trim();

            let (ty, data) = result.split_once(' ').unwrap_or((result, ""));

            let ev = match ty {
                "new_window" => {
                    let window_id = data.parse::<usize>().unwrap_or_else(|_| {
                        panic!("the window manager sent invalid new_window data: `{result}`")
                    });

                    Event::NewWindow { id: window_id }
                }
                _ => {
                    panic!("the window manager sent an unknown event: `{ty}`");
                }
            };

            if let Some(filter) = filter {
                if ty != filter {
                    self.event_buf.push(ev);
                    continue;
                }
            }

            return Ok(ev);
        }
    }
}
