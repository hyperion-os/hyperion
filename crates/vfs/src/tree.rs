use alloc::sync::{Arc, Weak};
use core::fmt;

use hyperion_log::*;
use hyperion_syscall::err::{Error, Result};
use lock_api::{Mutex, RawMutex};

use crate::{
    device::DirectoryDevice,
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

impl<Mut> Node<Mut> {
    pub fn try_as_file(&self) -> Result<FileRef<Mut>> {
        match self {
            Node::File(f) => Ok(f.clone()),
            Node::Directory(_) => Err(Error::NOT_A_FILE),
        }
    }

    pub fn try_as_dir(&self) -> Result<DirRef<Mut>> {
        match self {
            Node::File(_) => Err(Error::NOT_A_DIRECTORY),
            Node::Directory(d) => Ok(d.clone()),
        }
    }
}

impl<Mut: RawMutex> fmt::Debug for Node<Mut> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Node::File(v) => f.debug_tuple("File").field(&v.lock().driver()).finish(),
            Node::Directory(v) => f
                .debug_tuple("Directory")
                .field(&v.lock().driver())
                .finish(),
        }
    }
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

    pub fn new_file(f: impl FileDevice + 'static) -> Self {
        Self::File(Arc::new(Mutex::new(f)))
    }

    pub fn new_dir(f: impl DirectoryDevice<Mut> + 'static) -> Self {
        Self::Directory(Arc::new(Mutex::new(f)))
    }

    pub fn find(&self, path: impl AsRef<Path>, make_dirs: bool) -> Result<Self> {
        let mut this = self.clone();
        for part in path.as_ref().iter() {
            let mut dir = this.try_as_dir()?.lock_arc();

            // TODO: only Node::Directory should be cloned

            this = if let Ok(node) = dir.get_node(part) {
                node
            } else if make_dirs {
                let node = Self::Directory(Directory::new_ref(part));
                dir.create_node(part, node.clone())?;
                node
            } else {
                return Err(Error::NOT_FOUND);
            };
        }

        Ok(this)
    }

    pub fn find_dir(
        &self,
        path: impl AsRef<Path>,
        make_dirs: bool,
        create: bool,
    ) -> Result<DirRef<Mut>> {
        let path = path.as_ref();
        let (parent, target_dir) = path.split();

        if path.is_root() {
            return self.try_as_dir();
        }

        let parent = self.find(parent, make_dirs)?.try_as_dir()?;

        if target_dir.is_empty() {
            return Ok(parent);
        }

        let mut parent = parent.lock();

        // existing dir
        if let Ok(found) = parent.get_node(target_dir) {
            return found.try_as_dir();
        }

        // new file
        if create {
            let node = Directory::new_ref(target_dir);
            parent.create_node(target_dir, Node::Directory(node.clone()))?;
            return Ok(node);
        }

        Err(Error::NOT_FOUND)
    }

    pub fn find_file(
        &self,
        path: impl AsRef<Path>,
        make_dirs: bool,
        create: bool,
    ) -> Result<FileRef<Mut>> {
        let path = path.as_ref();
        let (parent, file) = path.split();

        let mut parent = self.find(parent, make_dirs)?.try_as_dir()?.lock_arc();

        // existing file
        if let Ok(found) = parent.get_node(file) {
            return found.try_as_file();
        }

        // new file
        if create {
            let node = File::new_empty();
            parent.create_node(file, Node::File(node.clone()))?;
            return Ok(node);
        }

        Err(Error::NOT_FOUND)
    }

    pub fn install_dev_with(&self, path: impl AsRef<Path>, dev: impl FileDevice + 'static) {
        self.install_dev_ref(path, Arc::new(Mutex::new(dev)) as _);
    }

    pub fn insert_file(
        &self,
        path: impl AsRef<Path>,
        make_dirs: bool,
        dev: FileRef<Mut>,
    ) -> Result<()> {
        self.insert(path, make_dirs, Node::File(dev))
    }

    pub fn insert_dir(
        &self,
        path: impl AsRef<Path>,
        make_dirs: bool,
        dev: DirRef<Mut>,
    ) -> Result<()> {
        self.insert(path, make_dirs, Node::Directory(dev))
    }

    pub fn insert(&self, path: impl AsRef<Path>, make_dirs: bool, node: Node<Mut>) -> Result<()> {
        let path = path.as_ref();
        let (parent_dir, target_name) = path.split();

        self.find_dir(parent_dir, make_dirs, true)?
            .lock()
            .create_node(target_name, node)
    }

    pub fn mount(&self, path: impl AsRef<Path>, dev: impl DirectoryDevice<Mut> + 'static) {
        self.mount_ref(path, Arc::new(Mutex::new(dev)))
    }

    pub fn mount_ref(&self, path: impl AsRef<Path>, dev: DirRef<Mut>) {
        let path = path.as_ref();
        trace!("mounting VFS device at {path:?}");
        if let Err(err) = self.insert_dir(path, true, dev) {
            error!("failed to mount VFS device at {path:?} : {err:?}");
        }
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
