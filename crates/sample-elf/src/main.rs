#![no_std]
#![no_main]
#![feature(format_args_nl, slice_internals)]

//

use libstd::{
    alloc::format,
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

        println!("got `{:?}`", core::str::from_utf8(&buf[..len]));

        if buf[..len].ends_with(b"2") {
            panic!()
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
