#![no_std]
#![feature(array_chunks, str_split_remainder)]

//

extern crate alloc;

use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use core::slice;

use libstd::println;
use spin::Once;

//

mod parse;

//

#[derive(Debug, Clone)]
enum Node {
    Dir(Dir),
    File(File),
}

#[derive(Debug, Clone)]
struct Dir {
    nodes: BTreeMap<Box<str>, Node>,
}

#[derive(Debug, Clone)]
struct File {
    data: Box<[u8]>,
}

//

static INITFS_ROOT: Once<Dir> = Once::new();

//

fn main() {
    println!("hello from bootstrap");

    // let initfs_arg = libstd::env::args()
    //     .find_map(|s| s.strip_prefix("initfs="))
    //     .expect("no initfs argument");

    // let (addr, size) = initfs_arg.split_once('+').expect("invalid initfs argument");
    // let (addr, size) = (
    //     usize::from_str_radix(addr.trim_start_matches("0x"), 16).expect("invalid addr"),
    //     usize::from_str_radix(size.trim_start_matches("0x"), 16).expect("invalid size"),
    // );

    let initfs = libstd::sys::sys_map_initfs().expect("failed to map initfs.tar.gz");

    println!("unpacking initfs");

    let initfs_tar_gz: &[u8] = unsafe { &*initfs };
    let tree = parse::parse_tar_gz(initfs_tar_gz);
    // println!("collected initfs: {tree:?}");

    INITFS_ROOT.call_once(move || tree);

    println!("/bin/init: {:?}", open("/bin/init"));

    // loop {
    //     libstd::sys::yield_now();
    // }
}

fn open(path: &str) -> Option<&[u8]> {
    let mut current = INITFS_ROOT.get().expect("initfs not initialized");

    let (parent_path, file_name) = path.rsplit_once('/').unwrap_or(("", path));

    for part in parent_path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }

        match current.nodes.get(part)? {
            Node::Dir(d) => current = d,
            _ => return None,
        }
    }

    let Node::File(file) = current.nodes.get(file_name)? else {
        return None;
    };

    Some(&file.data)
}
