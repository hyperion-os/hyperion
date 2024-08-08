#![no_std]

//

use libstd::println;

//

fn main() {
    println!("hello from bootstrap");

    let initfs_arg = libstd::env::args()
        .find_map(|s| s.strip_prefix("initfs="))
        .expect("no initfs argument");

    let (addr, size) = initfs_arg.split_once('+').expect("invalid initfs argument");
    let (addr, size) = (
        usize::from_str_radix(addr.trim_start_matches("0x"), 16).expect("invalid addr"),
        usize::from_str_radix(size.trim_start_matches("0x"), 16).expect("invalid size"),
    );

    println!("loading initfs from {addr:#x} ({size} bytes)");
}
