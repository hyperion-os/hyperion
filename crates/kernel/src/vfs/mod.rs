use alloc::{
    collections::{btree_map::Entry, BTreeMap},
    string::{String, ToString},
    sync::{Arc, Weak},
    vec::Vec,
};

use snafu::Snafu;
use spin::{Lazy, Mutex};

use self::path::Path;
use crate::{debug, error};

//

pub mod devices;
pub mod path;

//

static _ROOT_NODE: Lazy<Root> = Lazy::new(|| Directory::from(""));
pub static ROOT: Lazy<Root> = Lazy::new(|| {
    debug!("Initializing VFS");
    devices::install(Node::Directory(_ROOT_NODE.clone()));
    _ROOT_NODE.clone()
});

//

pub fn get_root() -> Node {
    Node::Directory(ROOT.clone())
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

pub fn install_dev(path: impl AsRef<Path>, dev: impl FileDevice + Send + Sync + 'static) {
    install_dev_with(get_root(), path, dev)
}

pub use get_dir as read_dir;
pub use get_file as open;

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
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize>;

    fn read_exact(&self, mut offset: usize, mut buf: &mut [u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.read(offset, buf) {
                Ok(0) => break,
                Ok(n) => {
                    offset += n;
                    let tmp = buf;
                    buf = &mut tmp[n..];
                }
                Err(IoError::Interrupted) => {}
                Err(err) => return Err(err),
            }
        }

        if !buf.is_empty() {
            Err(IoError::UnexpectedEOF)
        } else {
            Ok(())
        }
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize>;

    fn write_exact(&mut self, mut offset: usize, mut buf: &[u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.write(offset, buf) {
                Ok(0) => return Err(IoError::WriteZero),
                Ok(n) => {
                    offset += n;
                    buf = &buf[n..];
                }
                Err(IoError::Interrupted) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }
}

pub trait DirectoryDevice {
    fn get_node(&mut self, name: &str) -> IoResult<Node>;

    fn create_node(&mut self, name: &str, node: Node) -> IoResult<()>;

    fn nodes(&mut self) -> IoResult<Vec<String>>;
}

#[derive(Debug, Snafu)]
pub enum IoError {
    #[snafu(display("not found"))]
    NotFound,

    #[snafu(display("already exists"))]
    AlreadyExists,

    #[snafu(display("not a directory"))]
    NotADirectory,

    #[snafu(display("is a directory"))]
    IsADirectory,

    #[snafu(display("internal filesystem error"))]
    FilesystemError,

    #[snafu(display("permission denied"))]
    PermissionDenied,

    #[snafu(display("unexpected end of file"))]
    UnexpectedEOF,

    #[snafu(display("interrupted"))]
    Interrupted,

    #[snafu(display("wrote nothing"))]
    WriteZero,
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

impl IoError {
    pub fn to_str(&self) -> &'static str {
        match self {
            IoError::NotFound => "not found",
            IoError::AlreadyExists => "already exists",
            IoError::NotADirectory => "not a directory",
            IoError::IsADirectory => "is a directory",
            IoError::FilesystemError => "filesystem error",
            IoError::PermissionDenied => "permission denied",
            IoError::UnexpectedEOF => "unexpected eof",
            IoError::Interrupted => "interrupted",
            IoError::WriteZero => "wrote nothing",
        }
    }
}

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
    _create: bool,
) -> IoResult<FileRef> {
    let node = get_node_with(node, path, make_dirs)?;
    match node {
        Node::File(file) => Ok(file),
        Node::Directory(_) => Err(IoError::IsADirectory),
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

fn install_dev_with(
    node: Node,
    path: impl AsRef<Path>,
    dev: impl FileDevice + Send + Sync + 'static,
) {
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
