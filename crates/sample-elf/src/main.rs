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
        net::{Protocol, SocketDomain, SocketType},
        *,
    },
    thread::spawn,
    CliArgs,
};

//

mod io; // partial std::io

//

fn run_server() -> Result<()> {
    let server = socket(SocketDomain::LOCAL, SocketType::STREAM, Protocol::LOCAL)?;
    bind(server, "/dev/server.sock")?;

    rename("local server")?;

    let mut i = 0usize;
    loop {
        let conn = accept(server)?;
        println!("connected");

        let msg = format!("Hello {i}");
        i += 1;

        spawn(move || {
            send(conn, msg.as_bytes(), 0).unwrap();

            let mut buf = [0u8; 64];
            let len = recv(conn, &mut buf, 0).unwrap();
            assert_eq!(&buf[..len], b"ack");

            let file = OpenOptions::new()
                .read(true)
                .create(false)
                .open(format!("/tmp/{msg}"))
                .unwrap();

            let mut buf = [0u8; 64];
            let len = file.read(&mut buf).unwrap();
            assert_eq!(&buf[..len], b"testing data");

            drop(file); // drop = flush + close

            println!("server done");
        });
    }
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
            .open(format!("/tmp/{msg}"))
            .unwrap();

        file.write(b"testing data").unwrap();

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
    if run_server().is_err() {
        run_client().unwrap();
    }
}
