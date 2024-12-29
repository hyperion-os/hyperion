use core::mem::ManuallyDrop;

use core_alloc::{borrow::Cow, string::String};
use hyperion_syscall::{
    close,
    err::{Error, Result},
    fs::{FileDesc, FileOpenFlags, Metadata},
    metadata, open, read, write,
};

use crate::io::{self, BufReader};

//

pub fn create_dir(path: &str) -> Result<()> {
    OpenOptions::new()
        .read(true)
        .is_dir(true)
        .create(true)
        .create_dirs(false)
        .open(path)?;
    Ok(())
}

pub fn create_dir_all(path: &str) -> Result<()> {
    OpenOptions::new()
        .read(true)
        .is_dir(true)
        .create(true)
        .create_dirs(true)
        .open(path)?;
    Ok(())
}

//

pub struct Dir {
    file: BufReader<File>,
    cur: String,
}

impl Dir {
    pub fn open(path: &str) -> Result<Self> {
        Ok(Self {
            file: BufReader::new(OpenOptions::new().read(true).is_dir(true).open(path)?),
            cur: String::new(),
        })
    }

    pub fn next_entry(&mut self) -> Option<DirEntry> {
        self.cur.clear();
        self.file.read_line(&mut self.cur).ok()?;

        let mut iter = self.cur.trim().split(' ');
        let file_name = iter.next()?;
        let size = iter.next()?;
        let is_dir = iter.remainder()?;

        Some(DirEntry {
            is_dir: is_dir == "d",
            size: size.parse().unwrap(),
            file_name: Cow::Borrowed(file_name),
        })
    }
}

impl Iterator for Dir {
    type Item = DirEntry<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_entry().map(DirEntry::into_owned)
    }
}

//

#[derive(Debug)]
pub struct DirEntry<'a> {
    pub is_dir: bool, // TODO: mode flags later
    pub size: usize,
    pub file_name: Cow<'a, str>,
}

impl DirEntry<'_> {
    fn into_owned(self) -> DirEntry<'static> {
        DirEntry {
            is_dir: self.is_dir,
            size: self.size,
            file_name: Cow::Owned(self.file_name.into_owned()),
        }
    }
}

//

#[derive(Debug)]
pub struct File {
    desc: FileDesc,
    closed: bool,
}

impl File {
    /// # Safety
    ///
    /// `desc` must be a valid file descriptor
    ///
    /// this transfers the ownership of `desc` and will automatically close the file when dropped
    #[must_use]
    pub const unsafe fn new(desc: FileDesc) -> Self {
        Self {
            desc,
            closed: false,
        }
    }

    /// # Safety
    ///
    /// technically not unsafe, the fd should be closed at some point
    #[must_use]
    pub unsafe fn into_inner(self) -> FileDesc {
        ManuallyDrop::new(self).desc
    }

    /// # Safety
    ///
    /// file i/o won't be automatically synchronized
    #[must_use]
    pub const unsafe fn clone(&self) -> Self {
        Self {
            desc: self.desc,
            closed: self.closed,
        }
    }

    #[must_use]
    pub const fn as_desc(&self) -> FileDesc {
        self.desc
    }

    pub fn open(path: &str) -> Result<Self> {
        OpenOptions::new().read(true).write(true).open(path)
    }

    pub fn close(&mut self) -> Result<()> {
        if self.closed {
            return Err(Error::CLOSED);
        }
        self.closed = true;

        close(self.desc)
    }

    pub fn metadata(&self) -> Result<Metadata> {
        let mut meta = Metadata::zeroed();
        metadata(self.desc, &mut meta)?;
        Ok(meta)
    }
}

impl io::Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.closed {
            return Err(Error::CLOSED);
        }
        read(self.desc, buf)
    }
}

impl io::Write for File {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.closed {
            return Err(Error::CLOSED);
        }
        write(self.desc, buf)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Drop for File {
    fn drop(&mut self) {
        close(self.desc).expect("failed to close the file");
    }
}

//

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOptions {
    flags: FileOpenFlags,
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenOptions {
    #[must_use]
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

    pub fn is_dir(&mut self, is_dir: bool) -> &mut Self {
        self.flags.set(FileOpenFlags::IS_DIR, is_dir);
        self
    }

    pub fn create_dirs(&mut self, create_dirs: bool) -> &mut Self {
        self.flags.set(FileOpenFlags::CREATE_DIRS, create_dirs);
        self
    }

    pub fn open(&self, path: impl AsRef<str>) -> Result<File> {
        let fd = open(path.as_ref(), self.flags, 0)?;
        Ok(unsafe { File::new(fd) })
    }
}
