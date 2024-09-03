//! bootstrap sets up initfs, loads some critical software from there and forks into initfsd and init
//!
//! ```
//! <bootstrap>
//!  |
//!  +- /sbin/vm
//!  +- /sbin/pm
//!  +- /sbin/vfs
//!  |
//!  +-----------+
//!  |           |
//! <initfsd>   /sbin/init
//!              |
//!             ...
//! ```

#![no_std]
#![feature(array_chunks, str_split_remainder, naked_functions)]

//

extern crate alloc;

use alloc::{boxed::Box, collections::btree_map::BTreeMap, vec};
use core::{fmt, slice};

use libstd::{
    println,
    sys::{self, rename, system, Grant, GrantId, MessagePayload, Pid},
};
use spin::Once;

//

mod parse;

//

#[derive(Clone)]
enum Node {
    Dir(Dir),
    File(File),
}

#[derive(Clone)]
struct Dir {
    nodes: BTreeMap<Box<str>, Node>,
}

#[derive(Clone)]
struct File {
    data: Box<[u8]>,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Node::Dir(dir) => f.debug_tuple("Dir").field(&dir.nodes).finish(),
            Node::File(file) => f.debug_tuple("File").field(&file.data.len()).finish(),
        }
    }
}

//

static INITFS_ROOT: Once<Dir> = Once::new();

//

fn main() {
    println!("bootstrap: hello world");

    let initfs_arg = libstd::env::args()
        .find_map(|s| s.strip_prefix("initfs="))
        .expect("no initfs argument");

    let (addr, size) = initfs_arg.split_once('+').expect("invalid initfs argument");
    let (addr, size) = (
        usize::from_str_radix(addr.trim_start_matches("0x"), 16).expect("invalid addr"),
        usize::from_str_radix(size.trim_start_matches("0x"), 16).expect("invalid size"),
    );

    println!("unpacking initfs");

    let initfs_tar_gz: &[u8] = unsafe { slice::from_raw_parts(addr as _, size) };
    let tree = parse::parse_tar_gz(initfs_tar_gz);
    // println!("initfs tree:\n{:#?}", Node::Dir(tree));

    INITFS_ROOT.call_once(move || tree);

    println!("initfs unpacked");

    if sys::fork() == 0 {
        rename("initfsd").unwrap();
        // initfsd
        loop {}
    } else {
        rename("init").unwrap();
        sys::exec_elf(open("/sbin/init").expect("no /sbin/init"), &[]).unwrap();
    }

    // TODO: now fork and start the initfs server in the new process and exec init from here

    // TODO: start init
    // println!("/sbin/init: {:?}", open("/sbin/init"));
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
