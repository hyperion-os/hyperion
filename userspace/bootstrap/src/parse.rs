//! parse initfs.tar.gz to generate a read-only filesystem tree

use alloc::{boxed::Box, collections::btree_map::BTreeMap, string::String, vec::Vec};
use core::{fmt::Write, str, write, writeln};

use crate::{Dir, File, Node};

//

pub fn parse_tar_gz(initfs_tar_gz: &[u8]) -> Dir {
    // println!("initfs.tar.gz:\n{}", hexdump(initfs_tar_gz));

    let initfs_tar = gunzip(initfs_tar_gz);
    // println!("initfs.tar:\n{}", hexdump(&initfs_tar));

    assert_eq!(
        &initfs_tar[257..][..8],
        &[b'u', b's', b't', b'a', b'r', 0x20, 0x20, 0],
        "initfs.tar should be a GNU tar"
    );

    let mut root = BTreeMap::new();

    let mut blocks = initfs_tar.array_chunks::<512>();
    while let Some(header) = blocks.next() {
        let header: &TarEntryHeaderRaw = unsafe { &*header.as_ptr().cast() };
        // println!(
        //     "tar file:{} ty:{:?} size:{}",
        //     header.name(),
        //     header.ty(),
        //     header.size()
        // );

        if header.ty() != Type::File {
            // idc about anything other than files in the initfs, directories are created automatically
            continue;
        }

        let file_blocks = header.size().div_ceil(512);
        let mut file_buf = Vec::with_capacity(file_blocks * 512);
        for _ in 0..file_blocks {
            let file_block = blocks.next().expect("invalid TAR file size");
            file_buf.extend_from_slice(file_block);
        }
        file_buf.truncate(header.size());

        let path = header.name();
        let (parent_path, file) = path.rsplit_once('/').unwrap_or(("", path));
        if file.is_empty() {
            continue;
        }

        let parent_dir = goto_dir(parent_path, &mut root);
        // keep the last found duplicate
        parent_dir.insert(
            Box::from(file),
            Node::File(File {
                data: file_buf.into_boxed_slice(),
            }),
        );
    }

    Dir { nodes: root }
}

fn goto_dir<'a, 'b>(
    path: &'b str,
    root: &'a mut BTreeMap<Box<str>, Node>,
    // ) -> (&'a mut BTreeMap<Box<str>, Node>, Option<&'b str>) {
) -> &'a mut BTreeMap<Box<str>, Node> {
    // let mut parts = path.split('/');

    if path.is_empty() {
        return root;
    }

    let (part, remainder) = path.split_once('/').unwrap_or((path, ""));

    if part == "." {
        return goto_dir(remainder, root);
    } else if part.is_empty() {
        return goto_dir(remainder, root);
    }

    let next_node = root.entry(Box::from(part)).or_insert_with(|| {
        Node::Dir(Dir {
            nodes: BTreeMap::new(),
        })
    });

    match next_node {
        Node::Dir(d) => goto_dir(remainder, &mut d.nodes),
        Node::File(_) => {
            panic!("is a file")
        }
    }

    // FIXME: tail recursion is needed before the new borrow checker, Rust refuses this valid code for lifetime reasons
    // let mut cur_path = root;
    // while let Some(part) = parts.next() {
    //     if part == "." {
    //         continue;
    //     } else if part.is_empty() {
    //         break;
    //     }

    //     let next_node = cur_path.entry(Box::from(part)).or_insert_with(|| {
    //         Node::Dir(Dir {
    //             nodes: BTreeMap::new(),
    //         })
    //     });

    //     // match next_node {
    //     //     Node::Dir(d) => cur_path = &mut d.nodes,
    //     //     Node::File(_) => {
    //     //         drop(next_node);
    //     //     }
    //     // }
    // }

    // // (cur_path, parts.remainder())
    // parts.remainder()
}

#[repr(C)]
struct TarEntryHeaderRaw {
    name: [u8; 100],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 12],
    modified: [u8; 12],
    checksum: [u8; 8],
    ty: u8,
    link: [u8; 100],
}

impl TarEntryHeaderRaw {
    fn name(&self) -> &str {
        let bytes = self.name.split(|b| *b == 0).next().unwrap();
        str::from_utf8(bytes).unwrap()
    }

    fn size(&self) -> usize {
        // the size is in octal, which makes no sense
        let bytes = self.size.split(|b| *b == 0).next().unwrap();
        let size_str_octal = str::from_utf8(bytes).unwrap();
        usize::from_str_radix(size_str_octal, 8).unwrap_or(0)
    }

    fn ty(&self) -> Type {
        match self.ty {
            0 => Type::File,
            b'0' => Type::File,
            b'5' => Type::Dir,
            other => todo!("file type {other}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Type {
    File,
    Dir,
}

fn gunzip(tar_gz: &[u8]) -> Vec<u8> {
    assert_eq!(
        tar_gz.get(0..3),
        Some(&[0x1f, 0x8b, 0x08][..]),
        "initfs.tar.gz has to be a gzip with DEFLATE"
    );

    assert_eq!(tar_gz.get(3), Some(&0x00), "expected 0 extra headers");

    let (_header, payload) = tar_gz.split_at(10);
    let (payload, _checksum) = payload.split_at(payload.len() - 8);
    // FIXME: verify checksum

    miniz_oxide::inflate::decompress_to_vec(payload).expect("invalid gzip payload")
}

#[allow(unused)]
fn hexdump(b: &[u8]) -> String {
    let mut str = String::new();
    for row in b.chunks(8) {
        for byte in row {
            write!(str, "{byte:02x} ").unwrap();
        }
        writeln!(str).unwrap();
    }
    str
}
