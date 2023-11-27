#![no_std]
#![no_main]
#![feature(format_args_nl, slice_internals)]

//

use core::{hint::spin_loop, str::from_utf8};

use libstd::{
    alloc::format,
    fs::OpenOptions,
    println,
    sys::{
        err::Result,
        net::{Protocol, SocketDesc, SocketDomain, SocketType},
        *,
    },
    thread::spawn,
    CliArgs,
};

//

fn run_server() -> Result<()> {
    let server = socket(SocketDomain::LOCAL, SocketType::STREAM, Protocol::LOCAL)?;
    bind(server, "/dev/server.sock")?;

    rename("local server")?;

    let mut i = 0usize;
    loop {
        let conn = accept(server)?;
        i += 1;

        spawn(move || _ = handle_client(i, conn));
    }
}

fn handle_client(i: usize, conn: SocketDesc) -> Result<()> {
    let msg = format!("Hello {i}");

    send(conn, msg.as_bytes(), 0)?;

    let mut buf = [0u8; 64];
    let len = recv(conn, &mut buf, 0)?;
    assert_eq!(&buf[..len], b"ack");

    let file = OpenOptions::new()
        .read(true)
        .create(false)
        .open(format!("/tmp/{msg}"))?;

    let mut buf = [0u8; 64];
    let len = file.read(&mut buf)?;
    assert_eq!(&buf[..len], b"testing data");

    Ok(())
}

fn run_client() -> Result<()> {
    let client = socket(SocketDomain::LOCAL, SocketType::STREAM, Protocol::LOCAL)?;
    connect(client, "/dev/server.sock")?;

    rename("local client")?;

    println!("connected");

    loop {
        let mut buf = [0u8; 64];
        let len = recv(client, &mut buf, 0)?;

        let utf8 = &buf[..len];
        let msg = from_utf8(utf8).unwrap();
        println!("got `{msg:?}`");

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(format!("/tmp/{msg}"))?;

        file.write(b"testing data")?;

        drop(file); // drop = flush + close

        if buf[..len].ends_with(b"2") {
            println!("infinite loop");
            loop {
                spin_loop();
            }
        }

        send(client, b"ack", 0)?;
    }
}

#[no_mangle]
pub fn main(_args: CliArgs) {
    println!("PID:{} TID:{}", get_pid(), get_tid());

    if run_server().is_err() {
        if let Err(err) = run_client() {
            println!("error: {err}")
        };
    }
}
