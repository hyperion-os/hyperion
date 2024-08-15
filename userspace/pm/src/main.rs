#![no_std]

//

extern crate alloc;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::mem::MaybeUninit;

use libstd::{
    println,
    sys::{self, Message, MessagePayload, Pid},
};

//

pub struct Process {
    command: String,
    addr_space: PhysAddr,
}

pub struct PhysAddr(pub usize);

//

fn main() {
    println!("PM: hello world");

    // FIXME: u32::MAX sized array using some page magic
    let mut proc_table = Vec::new();

    proc_table.push(Process {
        command: String::from("<kernel async>"), // TODO: this will be gone soon
        addr_space: PhysAddr(0),
    });

    proc_table.push(Process {
        command: String::from("<bootstrap>"),
        addr_space: PhysAddr(0),
    });
    proc_table.push(Process {
        command: String::from("<vm>"),
        addr_space: PhysAddr(0),
    });
    proc_table.push(Process {
        command: String::from("<pm>"),
        addr_space: PhysAddr(0),
    });

    let mut buf = Vec::new();

    loop {
        let msg: Message = sys::recv_msg(Pid::ANY).unwrap();
        println!("PM got msg: {msg:?}");

        match msg.payload {
            MessagePayload::ProcessManagerForkAndExec { grant, offs, size } => {
                buf.resize(size, 0);
                sys::grant_read(msg.from, grant, offs, &mut buf).unwrap();

                let mut simple_checksum = 0;
                for byte in &buf {
                    simple_checksum ^= byte;
                }

                println!("got bytes: {simple_checksum}");
            }
            _ => {}
        }
    }

    // libstd::net::LocalListener::bind();

    // libstd::fs::OpenOptions::new()
    //     .create_new(true)
    //     .read(true)
    //     .write(true)
    //     .open("pm://sock");

    // libstd::sys;
}
