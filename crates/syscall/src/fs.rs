use core::mem::ManuallyDrop;

use bitflags::bitflags;

use crate::{close, err::Result, open, read, write};

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileDesc(pub usize);

//

bitflags! {
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileOpenFlags: usize {
    /// open file with read caps
    const READ       = 0b000001;

    /// open file with write caps
    const WRITE      = 0b000010;

    /// open file with read and write caps
    const READ_WRITE = Self::READ.bits() | Self::WRITE.bits();

    /// writes append to the file
    const APPEND     = 0b000100;

    /// create file if it doesn't already exist
    const CREATE     = 0b001000;

    /// create file if it doesn't already exist and err if it already exists
    const CREATE_NEW = 0b010000;

    /// truncate file on open (if the file already existed)
    const TRUNC      = 0b100000;
}
}

//

pub struct File {
    desc: FileDesc,
}

impl File {
    /// # Safety
    ///
    /// `desc` must be a valid file descriptor
    ///
    /// this transfers the ownership of `desc` and will automatically close the file when dropped
    pub const unsafe fn new(desc: FileDesc) -> Self {
        Self { desc }
    }

    /// # Safety
    ///
    /// technically not unsafe, the fd should be closed at some point
    pub unsafe fn into_inner(self) -> FileDesc {
        ManuallyDrop::new(self).desc
    }

    pub fn open(path: &str) -> Result<Self> {
        OpenOptions::new().open(path)
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        read(self.desc, buf)
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        write(self.desc, buf)
    }

    pub fn close(&self) -> Result<()> {
        close(self.desc)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        self.close().expect("failed to close the file");
    }
}

//

pub struct OpenOptions {
    flags: FileOpenFlags,
    // read: bool,
    // write: bool,
    // append: bool,
    // create: bool,
    // create_new: bool,
    // truncate: bool,
}

impl OpenOptions {
    pub const fn new() -> Self {
        Self {
            flags: FileOpenFlags::empty(),
        }
    }

    pub fn read(&mut self, read: bool) -> &mut Self {
        self.flags.set(FileOpenFlags::READ, read);
        self
    }

    pub fn write(&mut self, write: bool) -> &mut Self {
        self.flags.set(FileOpenFlags::WRITE, write);
        self
    }

    pub fn append(&mut self, append: bool) -> &mut Self {
        self.flags.set(FileOpenFlags::APPEND, append);
        self
    }

    pub fn create(&mut self, create: bool) -> &mut Self {
        self.flags.set(FileOpenFlags::CREATE, create);
        self
    }

    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.flags.set(FileOpenFlags::CREATE_NEW, create_new);
        self
    }

    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.flags.set(FileOpenFlags::TRUNC, truncate);
        self
    }

    pub fn open(&self, path: &str) -> Result<File> {
        let fd = open(path, self.flags, 0)?;
        Ok(unsafe { File::new(fd) })
    }
}
