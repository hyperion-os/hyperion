use core::mem::ManuallyDrop;

use core_alloc::string::String;
use hyperion_syscall::{
    close,
    err::Result,
    fs::{FileDesc, FileOpenFlags, Metadata},
    metadata, open, open_dir, read, write,
};
use spin::{Mutex, MutexGuard};

use crate::io::{BufReader, BufWriter, ConstBufReader};

//

// static STDIN: File = unsafe { File::new(FileDesc(0)) };
// static STDOUT: File = unsafe { File::new(FileDesc(1)) };
// static STDERR: File = unsafe { File::new(FileDesc(2)) };

pub static STDIN: Stdin = {
    static mut STDIN_BUF: [u8; 4096] = [0u8; 4096];
    Stdin(Mutex::new(ConstBufReader::new(
        unsafe { File::new(Stdin::FD) },
        unsafe { &mut STDIN_BUF },
    )))
};

pub static STDOUT: Stdout = Stdout(Mutex::new(BufWriter::new(unsafe { File::new(Stdout::FD) })));

pub static STDERR: Stderr = Stderr(Mutex::new(BufWriter::new(unsafe { File::new(Stderr::FD) })));

//

pub struct Stdin(Mutex<ConstBufReader<'static, File>>);

impl Stdin {
    pub const FD: FileDesc = FileDesc(0);

    pub fn lock(&self) -> MutexGuard<ConstBufReader<'static, File>> {
        self.0.lock()
    }
}

//

pub struct Stdout(Mutex<BufWriter<File>>);

impl Stdout {
    pub const FD: FileDesc = FileDesc(1);

    pub fn lock(&self) -> MutexGuard<BufWriter<File>> {
        self.0.lock()
    }
}

//

pub struct Stderr(Mutex<BufWriter<File>>);

impl Stderr {
    pub const FD: FileDesc = FileDesc(2);

    pub fn lock(&self) -> MutexGuard<BufWriter<File>> {
        self.0.lock()
    }
}

//

pub struct Dir {
    file: BufReader<File>,
    cur: String,
}

impl Dir {
    pub fn open(path: &str) -> Result<Self> {
        Ok(Self {
            file: BufReader::new(unsafe { File::new(open_dir(path)?) }),
            cur: String::new(),
        })
    }

    pub fn next_entry(&mut self) -> Option<DirEntry> {
        self.cur.clear();
        self.file.read_line(&mut self.cur).ok()?;

        let mut iter = self.cur.trim().split(' ');
        let is_dir = iter.next()?;
        let size = iter.next()?;
        let file_name = iter.remainder()?;

        Some(DirEntry {
            is_dir: is_dir == "d",
            size: size.parse().unwrap(),
            file_name,
        })
    }
}

//

pub struct DirEntry<'a> {
    pub is_dir: bool, // TODO: mode flags later
    pub size: usize,
    pub file_name: &'a str,
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

    pub fn metadata(&self) -> Result<Metadata> {
        let mut meta = Metadata::zeroed();
        crate::println!("metadata at {:#x}", &mut meta as *mut _ as usize);
        metadata(self.desc, &mut meta)?;
        crate::println!("metadata {meta:?}");
        Ok(meta)
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
