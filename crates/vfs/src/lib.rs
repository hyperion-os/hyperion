#![no_std]

//

extern crate alloc;

use alloc::sync::Arc;

use hyperion_log::{debug, error};
use spin::{Lazy, Mutex};

use self::{
    device::FileDevice,
    error::{IoError, IoResult},
    path::Path,
    ramdisk::{Directory, File},
    tree::{DirRef, FileRef, Node},
};
use crate::tree::Root;

//

pub mod device;
pub mod error;
pub mod path;
pub mod ramdisk;
pub mod tree;

//

pub fn get_root() -> Node {
    pub static ROOT: Lazy<Root> = Lazy::new(|| Directory::new_ref(""));
    let root = ROOT.clone();

    device::init();

    Node::Directory(root)
}

pub fn get_node(path: impl AsRef<Path>, make_dirs: bool) -> IoResult<Node> {
    get_node_with(get_root(), path, make_dirs)
}

pub fn get_dir(path: impl AsRef<Path>, make_dirs: bool) -> IoResult<DirRef> {
    get_dir_with(get_root(), path, make_dirs)
}

// TODO: create
pub fn get_file(path: impl AsRef<Path>, make_dirs: bool, create: bool) -> IoResult<FileRef> {
    get_file_with(get_root(), path, make_dirs, create)
}

pub fn create_device(path: impl AsRef<Path>, make_dirs: bool, dev: FileRef) -> IoResult<()> {
    create_device_with(get_root(), path, make_dirs, dev)
}

pub fn install_dev(path: impl AsRef<Path>, dev: impl FileDevice + 'static) {
    install_dev_with(get_root(), path, dev)
}

pub use get_dir as read_dir;
pub use get_file as open;

//

fn get_node_with(mut node: Node, path: impl AsRef<Path>, make_dirs: bool) -> IoResult<Node> {
    for part in path.as_ref().iter() {
        match node {
            Node::File(_) => return Err(IoError::NotADirectory),
            Node::Directory(_dir) => {
                let mut dir = _dir.lock();
                // TODO: only Node::Directory should be cloned

                node = if let Ok(node) = dir.get_node(part) {
                    node
                } else if make_dirs {
                    let node = Node::Directory(Directory::new_ref(part));
                    dir.create_node(part, node.clone())?;
                    node
                } else {
                    return Err(IoError::NotFound);
                };
            }
        }
    }

    Ok(node)
}

fn get_dir_with(node: Node, path: impl AsRef<Path>, make_dirs: bool) -> IoResult<DirRef> {
    let node = get_node_with(node, path, make_dirs)?;
    match node {
        Node::File(_) => Err(IoError::NotADirectory),
        Node::Directory(dir) => Ok(dir),
    }
}

fn get_file_with(
    node: Node,
    path: impl AsRef<Path>,
    make_dirs: bool,
    create: bool,
) -> IoResult<FileRef> {
    let path = path.as_ref();
    let (parent, file) = path.split().ok_or(IoError::NotFound)?;
    let node = get_node_with(node, parent, make_dirs)?;
    match node {
        Node::File(_) => Err(IoError::NotADirectory),
        Node::Directory(parent) => {
            let mut parent = parent.lock();

            // existing file
            match parent.get_node(file) {
                Ok(Node::File(file)) => return Ok(file),
                Ok(Node::Directory(_)) => return Err(IoError::IsADirectory),
                Err(_) => {}
            }

            // new file
            if create {
                let node = File::new_empty();
                parent.create_node(file, Node::File(node.clone()))?;
                return Ok(node);
            }

            Err(IoError::NotFound)
        }
    }
}

fn create_device_with(
    node: Node,
    path: impl AsRef<Path>,
    make_dirs: bool,
    dev: FileRef,
) -> IoResult<()> {
    create_node_with(node, path, make_dirs, Node::File(dev))
}

fn install_dev_with(node: Node, path: impl AsRef<Path>, dev: impl FileDevice + 'static) {
    let path = path.as_ref();
    debug!("installing VFS device at {path:?}");
    if let Err(err) = create_device_with(node, path, true, Arc::new(Mutex::new(dev)) as _) {
        error!("failed to install VFS device at {path:?} : {err:?}");
    }
}

fn create_node_with(
    root: Node,
    path: impl AsRef<Path>,
    make_dirs: bool,
    node: Node,
) -> IoResult<()> {
    let (parent_dir, file_name) = path.as_ref().split().ok_or(IoError::NotFound)?;
    let parent_dir = get_dir_with(root, parent_dir, make_dirs)?;

    let mut parent_dir = parent_dir.lock();
    parent_dir.create_node(file_name, node)?;

    Ok(())
}

/* fn create_node(path: impl AsRef<Path>, make_dirs: bool, node: Node) -> IoResult<()> {
    create_node_with(Node::Directory(ROOT.clone()), path, make_dirs, node)
} */
