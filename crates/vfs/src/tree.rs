use alloc::sync::{Arc, Weak};

use spin::Mutex;

use crate::{device::DirectoryDevice, FileDevice};

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

pub type FileRef = Arc<Mutex<dyn FileDevice + 'static>>;
pub type WeakFileRef = Weak<Mutex<dyn FileDevice + 'static>>;
pub type DirRef = Arc<Mutex<dyn DirectoryDevice + 'static>>;
pub type WeakDirRef = Weak<Mutex<dyn DirectoryDevice + 'static>>;
pub type Root = DirRef;
