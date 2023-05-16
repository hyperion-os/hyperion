use alloc::{
    collections::{btree_map::Entry, BTreeMap},
    string::{String, ToString},
    sync::{Arc, Weak},
    vec::Vec,
};
use spin::{Lazy, Mutex};

use crate::{debug, error};

use self::path::Path;

//

pub mod path;

//

pub static ROOT: Lazy<Root> = Lazy::new(|| Directory::from(""));

//

pub fn get_node(path: impl AsRef<Path>, make_dirs: bool) -> IoResult<Node> {
    let mut node = Node::Directory(ROOT.clone());

    for part in path.as_ref().iter() {
        match node {
            Node::File(_) => return Err(IoError::NotADirectory),
            Node::Directory(_dir) => {
                let mut dir = _dir.lock();
                // TODO: only Node::Directory should be cloned

                node = if let Ok(node) = dir.get_node(part) {
                    node
                } else if make_dirs {
                    let node = Node::Directory(Directory::from(part));
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

pub fn get_dir(path: impl AsRef<Path>, make_dirs: bool) -> IoResult<DirRef> {
    let node = get_node(path, make_dirs)?;
    match node {
        Node::File(_) => Err(IoError::NotADirectory),
        Node::Directory(dir) => Ok(dir),
    }
}

// TODO: create
pub fn get_file(path: impl AsRef<Path>, make_dirs: bool, _create: bool) -> IoResult<FileRef> {
    let node = get_node(path, make_dirs)?;
    match node {
        Node::File(file) => Ok(file),
        Node::Directory(_) => Err(IoError::IsADirectory),
    }
}

pub fn create_device(path: impl AsRef<Path>, make_dirs: bool, dev: FileRef) -> IoResult<()> {
    create_node(path, make_dirs, Node::File(dev))
}

pub fn install_dev(path: impl AsRef<Path>, dev: impl FileDevice + Send + Sync + 'static) {
    let path = path.as_ref();
    debug!("installing VFS device at {path:?}");
    if let Err(err) = create_device(path, true, Arc::new(Mutex::new(dev)) as _) {
        error!("failed to install VFS device at {path:?} : {err:?}");
    }
}

pub use {get_dir as read_dir, get_file as open};

//

#[derive(Clone)]
pub enum Node {
    /// a normal file, like `/etc/fstab`
    ///
    /// or
    ///
    /// a device mapped to a file, like `/dev/fb0`
    File(FileRef),

    /// a directory with 0 or more files, like `/home/`
    ///
    /// or
    ///
    /// a device mapped to a directory, like `/https/archlinux/org/`
    ///
    /// mountpoints are also directory devices
    ///
    /// directory devices may be unlistable, because it's not sensible to list every website there
    /// is
    ///
    /// a directory device most likely contains more directory devices, like `/https/archlinux/org`
    /// inside `/https/archlinux/`
    Directory(DirRef),
}

pub type FileRef = Arc<Mutex<dyn FileDevice + Sync + Send + 'static>>;
pub type WeakFileRef = Weak<Mutex<dyn FileDevice + Sync + Send + 'static>>;
pub type DirRef = Arc<Mutex<dyn DirectoryDevice + Sync + Send + 'static>>;
pub type WeakDirRef = Weak<Mutex<dyn DirectoryDevice + Sync + Send + 'static>>;
pub type Root = DirRef;

pub struct File {}

pub struct Directory {
    pub name: String,
    pub children: BTreeMap<String, Node>,
    pub parent: Option<WeakDirRef>,
}

pub trait FileDevice {
    fn len(&mut self) -> usize;

    fn is_empty(&mut self) -> bool {
        self.len() == 0
    }

    fn read(&mut self, offset: usize, buf: &mut [u8]) -> IoResult<usize>;

    fn read_exact(&mut self, offset: usize, buf: &mut [u8]) -> IoResult<()>;

    fn write(&mut self, offset: usize, bytes: &mut [u8]) -> IoResult<usize>;

    fn write_exact(&mut self, offset: usize, bytes: &mut [u8]) -> IoResult<()>;
}

pub trait DirectoryDevice {
    fn get_node(&mut self, name: &str) -> IoResult<Node>;

    fn create_node(&mut self, name: &str, node: Node) -> IoResult<()>;

    fn nodes(&mut self) -> IoResult<Vec<String>>;
}

#[derive(Debug)]
pub enum IoError {
    NotFound,
    AlreadyExists,
    NotADirectory,
    IsADirectory,
    FilesystemError,
    PermissionDenied,
    UnexpectedEOF,
}

pub type IoResult<T> = Result<T, IoError>;

//

impl DirectoryDevice for Directory {
    fn get_node(&mut self, name: &str) -> IoResult<Node> {
        if let Some(node) = self.children.get(name) {
            Ok(node.clone())
        } else {
            Err(IoError::NotFound)
        }
    }

    fn create_node(&mut self, name: &str, node: Node) -> IoResult<()> {
        match self.children.entry(name.to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(node);
                Ok(())
            }
            Entry::Occupied(_) => Err(IoError::AlreadyExists),
        }
    }

    fn nodes(&mut self) -> IoResult<Vec<String>> {
        Ok(self.children.keys().cloned().collect())
    }
}

impl Directory {
    pub fn from(name: impl Into<String>) -> DirRef {
        Arc::new(Mutex::new(Directory {
            name: name.into(),
            children: BTreeMap::new(),
            parent: None,
        })) as _
    }
}

//

fn create_node(path: impl AsRef<Path>, make_dirs: bool, node: Node) -> IoResult<()> {
    let (parent_dir, file_name) = path.as_ref().split().ok_or(IoError::NotFound)?;
    let parent_dir = get_dir(parent_dir, make_dirs)?;

    let mut parent_dir = parent_dir.lock();
    parent_dir.create_node(file_name, node)?;

    Ok(())
}
