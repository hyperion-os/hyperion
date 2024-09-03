#![no_std]

//

use libstd::sys::system;

//

fn main() {
    libstd::sys::rename("init");

    libstd::println!("init: hello world");

    system("/bin/wm", &[]);
}
