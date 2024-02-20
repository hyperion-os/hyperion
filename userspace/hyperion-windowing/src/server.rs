use std::{
    fs::{self, File},
    io::{self, BufReader, Seek, SeekFrom, Write},
    ptr::{self, NonNull},
    sync::Arc,
    thread,
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use hyperion_syscall::{fs::FileDesc, map_file};

use crate::{
    os::{AsRawFd, LocalListener, LocalStream},
    shared::{ConnectionClosed, Message, Request},
};

//

pub struct Server {
    listener: LocalListener,
}

impl Server {
    pub fn new() -> io::Result<Self> {
        fs::create_dir_all("/run")?;

        Ok(Self {
            listener: LocalListener::bind("/run/wm.socket")?,
        })
    }

    pub fn accept(&self) -> io::Result<Connection> {
        let conn = Arc::new(self.listener.accept()?);
        let result_stream = MessageStream { conn: conn.clone() };
        let socket_r = BufReader::new(conn);

        let (request_buf_tx, request_buf) = unbounded();

        thread::spawn(move || handle_client(socket_r, request_buf_tx));

        Ok(Connection {
            request_buf,
            message_stream: result_stream,
        })
    }
}

//

pub struct Connection {
    request_buf: Receiver<Request>,
    message_stream: MessageStream,
}

impl Connection {
    pub fn next_request(&self) -> Result<Request, ConnectionClosed> {
        self.request_buf.recv().map_err(|_| ConnectionClosed)
    }

    pub fn send_message(&self, res: Message) -> Result<(), ConnectionClosed> {
        self.message_stream.send_message(res)
    }

    pub fn clone_tx(&self) -> MessageStream {
        self.message_stream.clone()
    }
}

//

#[derive(Clone)]
pub struct MessageStream {
    conn: Arc<LocalStream>,
}

impl MessageStream {
    pub fn send_message(&self, msg: Message) -> Result<(), ConnectionClosed> {
        rmp_serde::encode::write(&mut &*self.conn, &msg).map_err(|_| ConnectionClosed)
    }
}

//

pub fn new_window_framebuffer(
    pitch: usize,
    height: usize,
    window_id: usize,
) -> (File, NonNull<u32>) {
    // TODO: anonymous file + pass the fd instead of making a file that any proc can read
    let path = format!("/run/wm.window.{window_id}");
    // TODO: create_new
    let mut window_file = File::create(path.as_str()).unwrap();
    // TODO: truncate
    window_file
        .seek(SeekFrom::Start((pitch * height * 4 - 4) as u64))
        .unwrap();
    window_file.write_all(&[0u8; 4]).unwrap();
    let len = window_file.metadata().unwrap().len() as usize;

    let shmem_ptr: NonNull<u32> = map_file(FileDesc(window_file.as_raw_fd()), None, len, 0)
        .unwrap()
        .cast();
    // println!("shmem_ptr={:#018x}", shmem_ptr.as_ptr() as usize);
    let shmem = ptr::slice_from_raw_parts_mut(shmem_ptr.as_ptr().cast::<u8>(), len);
    // TODO: volatile
    let shmem = unsafe { &mut *shmem };
    shmem.fill(0);

    (window_file, shmem_ptr)
}

fn handle_client(mut socket_r: BufReader<Arc<LocalStream>>, request_buf_tx: Sender<Request>) {
    loop {
        let Ok(req) = rmp_serde::from_read(&mut socket_r) else {
            eprintln!("invalid request from a client, closing the connection");
            break;
        };

        if request_buf_tx.send(req).is_err() {
            eprintln!("server closed");
            // server closed
            break;
        }
    }
}
