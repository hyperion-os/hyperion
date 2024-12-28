#![no_std]
#![feature(slice_as_chunks)]

//

extern crate alloc;

use alloc::{format, string::String, sync::Arc};
use core::str::from_utf8;

use libstd::{
    fs::{self, File},
    io::{stdout, BufReader, Read, Stdin, Stdout, Write},
    net::{LocalListener, LocalStream},
    println,
    sync::Mutex,
    sys::{
        err::Result,
        fs::{FileDesc, FileOpenFlags},
        *,
    },
    thread::spawn,
};

//

fn run_server() -> Result<()> {
    fs::create_dir_all("/run").unwrap();

    let server = LocalListener::bind("/run/server.sock")?;

    // close(Stdin::FD).unwrap();
    // close(Stdout::FD).unwrap();
    let null = open("/dev/null", FileOpenFlags::READ_WRITE, 0).unwrap();
    dup(null, Stdin::FD).unwrap();
    dup(null, Stdout::FD).unwrap();

    rename("local server")?;

    let mut i = 0usize;
    loop {
        let conn = server.accept()?;
        i += 1;

        spawn(move || _ = handle_client(i, conn));
    }
}

fn handle_client(i: usize, mut conn: LocalStream) -> Result<()> {
    let msg = format!("Hello {i}");

    conn.write(msg.as_bytes())?;

    let mut buf = [4u8; 4];
    let len = conn.read(&mut buf)?;
    assert_eq!(&buf[..len], b"ack");

    // let mut file = OpenOptions::new()
    //     .read(true)
    //     .create(false)
    //     .open(format!("/tmp/{msg}"))?;

    // let mut buf = [0u8; 13];
    // let len = file.read(&mut buf)?;
    // assert_eq!(&buf[..len], b"testing data");

    Ok(())
}

fn run_client() -> Result<()> {
    let mut client = LocalStream::connect("/run/server.sock")?;

    rename("local client")?;
    println!("connected");

    loop {
        let mut buf = [0u8; 64];
        let len = client.read(&mut buf)?;

        if len == 0 {
            break Ok(());
        }

        let utf8 = &buf[..len];
        let msg = from_utf8(utf8).unwrap();
        println!("got `{msg:?}`");

        // let mut file = OpenOptions::new()
        //     .write(true)
        //     .create(true)
        //     .open(format!("/tmp/{msg}"))?;

        // file.write(b"testing data")?;
        // drop(file); // drop = flush + close

        client.write(b"ack")?;
    }
}

fn _test_duplicate_stdin() -> File {
    let dup = dup(Stdin::FD, FileDesc(10)).unwrap();
    let stdin_dupe = unsafe { File::new(dup) };
    close(Stdin::FD).unwrap();
    stdin_dupe
}

fn _test_userspace_mutex() {
    let counter = Arc::new(Mutex::new(0usize));
    for _ in 0..100 {
        let counter = counter.clone();
        spawn(move || {
            *counter.lock() += 1;
        });
    }

    loop {
        if *counter.lock() == 100 {
            break;
        }

        yield_now();
    }

    println!("complete");
}

fn _repeat_stdin_to_stdout() {
    let mut stdin = _test_duplicate_stdin();

    let mut reader = BufReader::new(&mut stdin);
    // let mut reader = BufReader::new(&STDIN);
    let mut buf = String::new();
    loop {
        buf.clear();
        let len = reader.read_line(&mut buf).unwrap();

        if len == 0 {
            break;
        }

        stdout().lock().write(buf.as_bytes()).unwrap();
    }
}

pub fn main() -> Result<()> {
    // _test_userspace_mutex();
    // _repeat_stdin_to_stdout();

    libstd::sys::log!("hello world");

    let value = get_pid();
    println!("PID:{} TID:{}", get_pid(), get_tid());

    let fork_result = fork();

    println!(
        "fork_result:{fork_result:?} value={value} PID:{} TID:{}",
        get_pid(),
        get_tid()
    );

    _ = run_server();
    nanosleep(100000000);
    run_client()
}
