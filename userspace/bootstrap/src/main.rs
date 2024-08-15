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
#![feature(array_chunks, str_split_remainder)]

//

extern crate alloc;

use alloc::{boxed::Box, collections::btree_map::BTreeMap, vec};
use core::fmt;

use libstd::{
    println,
    sys::{self, Grant, GrantId, MessagePayload, Pid},
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
    println!("bootstrap: hello from");

    let initfs = sys::sys_map_initfs().expect("failed to map initfs.tar.gz");

    println!("unpacking initfs");

    let initfs_tar_gz: &[u8] = unsafe { &*initfs };
    let tree = parse::parse_tar_gz(initfs_tar_gz);
    // println!("initfs tree:\n{:#?}", Node::Dir(tree));

    INITFS_ROOT.call_once(move || tree);

    // FIXME: map the stack from here
    // FIXME: map the ELFs from here

    // TODO: start VM
    sys::sys_bootstrap_provide_vm(open("/sbin/vm").expect("initfs doesn't have vm"))
        .expect("could not start vm");

    // TODO: start PM
    sys::sys_bootstrap_provide_pm(open("/sbin/pm").expect("initfs doesn't have pm"))
        .expect("could not start pm");

    // TODO: start VFS
    let vfs = open("/sbin/vfs").expect("initfs doesn't have vfs");
    let mut simple_checksum = 0;
    for byte in vfs {
        simple_checksum ^= byte;
    }
    println!("sending bytes: {simple_checksum}");
    sys::set_grants(vec![Grant::new(Pid::PM, vfs, true, false)].leak());
    sys::send_msg(
        Pid::PM,
        MessagePayload::ProcessManagerForkAndExec {
            grant: GrantId(0),
            offs: 0,
            size: vfs.len(),
        },
    )
    .unwrap();
    sys::recv_msg(Pid::PM).unwrap();
    sys::set_grants(&[]);
    // sys::sys_bootstrap_provide_vfs().expect("could not start vfs");

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
