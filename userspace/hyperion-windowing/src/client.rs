use std::{
    fs::File,
    io::{self, BufReader},
    ptr::NonNull,
    sync::Arc,
    thread,
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use hyperion_syscall::{fs::FileDesc, map_file, unmap_file};

use crate::{
    global::Region,
    os::{AsRawFd, LocalStream},
    shared::{Event, Message, Request},
};

//

#[derive(Clone)]
pub struct Connection {
    inner: Arc<ConnectionInner>,
}

struct ConnectionInner {
    event_buf: Receiver<Event>,
    pending_windows: Receiver<usize>,

    socket_w: Arc<LocalStream>,
}

impl Connection {
    pub fn new() -> io::Result<Self> {
        let socket = Arc::new(LocalStream::connect("/run/wm.socket")?);
        let socket_r = BufReader::new(socket.clone());
        let socket_w = socket;

        let (event_buf_tx, event_buf) = unbounded();
        let (pending_windows_tx, pending_windows) = unbounded();

        thread::spawn(move || {
            conn_handler(socket_r, event_buf_tx, pending_windows_tx);
        });

        Ok(Self {
            inner: Arc::new(ConnectionInner {
                event_buf,
                pending_windows,
                socket_w,
            }),
        })
    }

    pub fn new_window(&self) -> io::Result<Window> {
        self.send_request(Request::NewWindow);

        let window_id = self.inner.pending_windows.recv().unwrap();

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
            // conn: self.clone(),
            // window_id,
            fbo,
            fbo_ptr,
            width: 400,
            height: 400,
            pitch: 400,
        })
    }

    pub fn next_event(&self) -> Event {
        self.inner.event_buf.recv().unwrap()
    }

    pub fn send_request(&self, req: Request) {
        rmp_serde::encode::write(&mut &*self.inner.socket_w, &req).unwrap();
        // (&*self.inner.socket_w).write_all(&[0]).unwrap();
    }
}

//

pub struct Window {
    // TODO: make each window a its own stream?
    // conn: Connection,
    // window_id: usize,
    fbo: File,
    fbo_ptr: NonNull<()>, // TODO: volatile write
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
}

impl Window {
    pub fn buf_base(&mut self) -> NonNull<u32> {
        self.fbo_ptr.cast()
    }

    pub fn fill(&mut self, color: u32) {
        let pixels = self.fbo_ptr.cast::<u32>().as_ptr();

        for y in 0..self.height {
            for x in 0..self.width {
                // Rust should vectorize this
                // fill doesn't work because this memory is volatile
                unsafe { pixels.add(x + y * self.pitch).write_volatile(color) };
            }
        }
    }

    pub fn as_region(&mut self) -> Region<'_> {
        let buf = self.fbo_ptr.as_ptr().cast();
        // SAFETY: Window is borrowed for the lifetime of Region<'_>
        // because GlobalFb owns the buffer mapping and automatically frees it
        unsafe { Region::new(buf, self.pitch, self.width, self.height) }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unmap_file(FileDesc(self.fbo.as_raw_fd()), self.fbo_ptr, 0)
            .expect("failed to unmap the fb");
    }
}

//

pub fn conn_handler(
    mut socket_r: BufReader<Arc<LocalStream>>,
    event_buf_tx: Sender<Event>,
    pending_windows_tx: Sender<usize>,
) {
    // let mut buf = [0u8; 256];

    loop {
        // if socket_r.read_until(0, &mut buf).unwrap() == 0 {
        //     break;
        // }
        // if buf.last() == Some(&0) {
        //     buf.pop();
        // }
        // if buf.len() == 0 {
        //     continue
        // }

        // let Ok(res) = rmp_serde::from_slice(&buf) else {
        //     eprintln!("invalid request from a server, closing the connection");
        //     break;
        // };

        let Ok(res) = rmp_serde::from_read(&mut socket_r) else {
            eprintln!("invalid request from a server, closing the connection");
            break;
        };

        let is_err = match res {
            Message::NewWindow { window_id } => pending_windows_tx.send(window_id).is_err(),
            Message::Event(ev) => event_buf_tx.send(ev).is_err(),
        };

        if is_err {
            // eprintln!("connection closed");
            // connection closed
            break;
        }
    }
}
