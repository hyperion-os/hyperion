use core::mem::ManuallyDrop;

use hyperion_syscall::{
    close,
    err::Result,
    fs::{FileDesc, FileOpenFlags},
    open, read, write,
};

//

pub static STDIN: File = unsafe { File::new(FileDesc(0)) };
pub static STDOUT: File = unsafe { File::new(FileDesc(1)) };
pub static STDERR: File = unsafe { File::new(FileDesc(2)) };

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

    /// # Safety
    ///
    /// file i/o won't be automatically synchronized
    pub const unsafe fn clone(&self) -> Self {
        Self { desc: self.desc }
    }

    pub const fn as_desc(&self) -> FileDesc {
        self.desc
    }

    pub fn open(path: &str) -> Result<Self> {
        OpenOptions::new().read(true).write(true).open(path)
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

    pub fn open(&self, path: impl AsRef<str>) -> Result<File> {
        let fd = open(path.as_ref(), self.flags, 0)?;
        Ok(unsafe { File::new(fd) })
    }
}
