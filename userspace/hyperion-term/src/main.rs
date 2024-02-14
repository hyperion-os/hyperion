#[cfg(not(feature = "cargo-clippy"))]
use std::os::hyperion::{net::LocalStream, AsRawFd};
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Write},
    ptr::NonNull,
    sync::{
        mpsc::{self},
        Arc,
    },
    thread,
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
    let mut i = get_pid();

    // let mut t = timestamp().unwrap() as u64;
    loop {
        window.fill(colors[i % 3]);

        match wm.next_event() {
            Event::Keyboard {} => i += 1,
        }

        // t += 16_666_667;
        // nanosleep_until(t);
    }
}

#[allow(unused)]
pub struct Window {
    // TODO: make each window a its own stream?
    conn: Connection,
    window_id: usize,

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

//

pub enum Event {
    Keyboard {},
}

#[derive(Clone)]
pub struct Connection {
    inner: Arc<ConnectionInner>,
}

struct ConnectionInner {
    read: ConnectionRead,
    write: ConnectionWrite,
}

struct ConnectionRead {
    event_buf: mpsc::Receiver<Event>,
    pending_windows: mpsc::Receiver<usize>,
}

struct ConnectionWrite {
    socket_w: Arc<LocalStream>,
}

impl Connection {
    pub fn new() -> io::Result<Self> {
        let socket = Arc::new(LocalStream::connect("/run/wm.socket")?);
        let socket_r = BufReader::new(socket.clone());
        let socket_w = socket;

        let (event_buf_tx, event_buf) = mpsc::channel();
        let (pending_windows_tx, pending_windows) = mpsc::channel();

        thread::spawn(move || {
            conn_reader(socket_r, event_buf_tx, pending_windows_tx);
        });

        Ok(Self {
            inner: Arc::new(ConnectionInner {
                read: ConnectionRead {
                    event_buf,
                    pending_windows,
                },
                write: ConnectionWrite { socket_w },
            }),
        })
    }

    pub fn new_window(&self) -> io::Result<Window> {
        println!("requesting window");
        self.inner.cmd("new_window")?;

        println!("waiting for new_window");
        let window_id = self.inner.read.pending_windows.recv().unwrap();
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
            window_id,
            fbo,
            fbo_ptr,
            width: 200,
            height: 200,
            pitch: 200,
        })
    }

    pub fn next_event(&self) -> Event {
        self.inner.next_event()
    }
}

impl ConnectionInner {
    pub fn cmd(&self, str: &str) -> io::Result<()> {
        self.write.cmd(str)
    }

    pub fn next_event(&self) -> Event {
        self.read.event_buf.recv().unwrap()
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
    pub fn next_event(&mut self) -> Event {
        self.event_buf.recv().unwrap()
    }
}

//

fn conn_reader(
    mut socket_r: BufReader<Arc<LocalStream>>,
    event_buf_tx: mpsc::Sender<Event>,
    pending_windows_tx: mpsc::Sender<usize>,
) {
    let mut buf = String::new();

    loop {
        // wait for the result
        buf.clear();
        let len = socket_r.read_line(&mut buf).unwrap();
        let result = buf[..len].trim();

        let (ty, data) = result.split_once(' ').unwrap_or((result, ""));

        match ty {
            "new_window" => {
                let window_id = data.parse::<usize>().unwrap_or_else(|_| {
                    panic!("the window manager sent invalid new_window data: `{result}`")
                });

                pending_windows_tx.send(window_id).unwrap();
            }
            "event" => {
                let (event_ty, _data) = data.split_once(' ').unwrap_or((data, ""));

                match event_ty {
                    "keyboard" => {
                        println!("kb event");
                        event_buf_tx.send(Event::Keyboard {}).unwrap()
                    }
                    _ => {
                        panic!("the window manager sent an unknown event: `{event_ty}`");
                    }
                }
            }
            _ => {
                panic!("the window manager sent an unknown msg: `{ty}`");
            }
        };

        _ = &event_buf_tx;

        // event_buf_tx.send(ev).unwrap();
    }
}
