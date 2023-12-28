use alloc::sync::{Arc, Weak};

use hyperion_log::*;
use lock_api::Mutex;

use crate::{
    device::DirectoryDevice,
    error::{IoError, IoResult},
    path::Path,
    ramdisk::{Directory, File},
    AnyMutex, FileDevice,
};

//

pub type FileRef<Mut> = Arc<Mutex<Mut, dyn FileDevice + 'static>>;
pub type WeakFileRef<Mut> = Weak<Mutex<Mut, dyn FileDevice + 'static>>;
pub type DirRef<Mut> = Arc<Mutex<Mut, dyn DirectoryDevice<Mut> + 'static>>;
pub type WeakDirRef<Mut> = Weak<Mutex<Mut, dyn DirectoryDevice<Mut> + 'static>>;
pub type Root<Mut> = DirRef<Mut>;

//

// pub type Ref<T, Mut: AnyMutex> = Arc<Mut::Mutex<T>>;

pub trait IntoRoot: Sized {
    type Mut: AnyMutex;

    fn into_root(self) -> Root<Self::Mut>;
}

impl<Mut: AnyMutex> IntoRoot for Root<Mut> {
    type Mut = Mut;

    fn into_root(self) -> Root<Self::Mut> {
        self
    }
}

//

pub trait IntoNode: Sized {
    type Mut: AnyMutex;

    fn into_node(self) -> Node<Self::Mut>;
}

impl<Mut: AnyMutex> IntoNode for Node<Mut> {
    type Mut = Mut;

    fn into_node(self) -> Node<Self::Mut> {
        self
    }
}

//

pub enum Node<Mut> {
    /// a normal file, like `/etc/fstab`
    ///
    /// or
    ///
    /// a device mapped to a file, like `/dev/fb0`
    File(FileRef<Mut>),

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
    Directory(DirRef<Mut>),
}

impl<Mut> Clone for Node<Mut> {
    fn clone(&self) -> Self {
        match self {
            Node::File(v) => Node::File(v.clone()),
            Node::Directory(v) => Node::Directory(v.clone()),
        }
    }
}

//

impl<Mut: AnyMutex> Node<Mut> {
    pub fn new_root() -> Self {
        Node::Directory(Directory::new_ref(""))
    }

    pub fn find(&self, path: impl AsRef<Path>, make_dirs: bool) -> IoResult<Self> {
        let mut this = self.clone();
        for part in path.as_ref().iter() {
            match this {
                Node::File(_) => return Err(IoError::NotADirectory),
                Node::Directory(_dir) => {
                    let mut dir = _dir.lock();
                    // TODO: only Node::Directory should be cloned

                    this = if let Ok(node) = dir.get_node(part) {
                        node
                    } else if make_dirs {
                        let node = Self::Directory(Directory::new_ref(part));
                        dir.create_node(part, node.clone())?;
                        node
                    } else {
                        return Err(IoError::NotFound);
                    };
                }
            }
        }

        Ok(this)
    }

    pub fn find_dir(&self, path: impl AsRef<Path>, make_dirs: bool) -> IoResult<DirRef<Mut>> {
        match self.find(path, make_dirs)? {
            Node::File(_) => Err(IoError::NotADirectory),
            Node::Directory(dir) => Ok(dir),
        }
    }

    pub fn find_file(
        &self,
        path: impl AsRef<Path>,
        make_dirs: bool,
        create: bool,
    ) -> IoResult<FileRef<Mut>> {
        let path = path.as_ref();
        let (parent, file) = path.split().ok_or(IoError::NotFound)?;

        match self.find(parent, make_dirs)? {
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

    pub fn install_dev_with(&self, path: impl AsRef<Path>, dev: impl FileDevice + 'static) {
        self.install_dev_ref(path, Arc::new(Mutex::new(dev)) as _);
    }

    pub fn insert_file(
        &self,
        path: impl AsRef<Path>,
        make_dirs: bool,
        dev: FileRef<Mut>,
    ) -> IoResult<()> {
        self.insert(path, make_dirs, Node::File(dev))
    }

    pub fn insert_dir(
        &self,
        path: impl AsRef<Path>,
        make_dirs: bool,
        dev: DirRef<Mut>,
    ) -> IoResult<()> {
        self.insert(path, make_dirs, Node::Directory(dev))
    }

    pub fn insert(&self, path: impl AsRef<Path>, make_dirs: bool, node: Node<Mut>) -> IoResult<()> {
        let (parent_dir, file_name) = path.as_ref().split().ok_or(IoError::NotFound)?;
        let parent_dir = self.find_dir(parent_dir, make_dirs)?;

        let mut parent_dir = parent_dir.lock();
        parent_dir.create_node(file_name, node)?;

        Ok(())
    }

    pub fn install_dev(&self, path: impl AsRef<Path>, dev: impl FileDevice + 'static) {
        self.install_dev_ref(path, Arc::new(Mutex::new(dev)) as _);
    }

    pub fn install_dev_ref(&self, path: impl AsRef<Path>, dev: FileRef<Mut>) {
        let path = path.as_ref();
        trace!("installing VFS device at {path:?}");
        if let Err(err) = self.insert_file(path, true, dev) {
            error!("failed to install VFS device at {path:?} : {err:?}");
        }
    }

    /* fn create_node(path: impl AsRef<Path>, make_dirs: bool, node: Node) -> IoResult<()> {
        create_node_with(Node::Directory(ROOT.clone()), path, make_dirs, node)
    } */
}
