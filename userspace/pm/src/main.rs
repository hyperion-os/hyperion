#![no_std]

//

use libstd::{
    println,
    sys::{self, Pid},
};

//

fn main() {
    println!("PM: hello world");

    loop {
        let msg = sys::recv_msg(Pid::ANY).unwrap();
        println!("PM got msg: {msg:?}")
    }

    // libstd::net::LocalListener::bind();

    // libstd::fs::OpenOptions::new()
    //     .create_new(true)
    //     .read(true)
    //     .write(true)
    //     .open("pm://sock");

    // libstd::sys;
}
