#![no_std]

//

use libstd::{
    println,
    sys::{self, fs::FileOpenFlags, open, system},
};

//

fn main() {
    sys::rename("init");

    println!("init: hello world");

    system("initfs:///sbin/vfs", &[]);
    system("/bin/wm", &[]);
}
