#![no_std]
#![feature(pointer_is_aligned)]

//

extern crate alloc;

use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};
use core::{
    any::Any,
    mem, slice,
    sync::atomic::{AtomicUsize, Ordering},
};

use arcstr::ArcStr;
use hyperion_loader::Loader;
use hyperion_log::*;
use hyperion_mem::vmm::PageMapImpl;
use hyperion_scheduler::{
    ipc::pipe::{pipe_with, Channel, Receiver, Sender},
    lock::{Futex, Mutex},
    proc::{Pid, Process, ProcessExt},
    process,
};
use hyperion_syscall::{
    err::{Error, Result},
    fs::{FileDesc, Seek},
    net::{Protocol, SocketDomain, SocketType},
};
use hyperion_vfs::{
    device::FileDevice,
    error::IoError,
    tree::{FileRef, Node},
};
use spin::{Lazy, Once};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

mod procfs;
// mod sysfs;

//

pub static VFS_ROOT: Lazy<Node<Futex>> = Lazy::new(|| {
    let root = Node::new_root();

    procfs::init(root.clone());
    // sysfs::init(root.clone());

    root
});

//

#[derive(Clone)]
pub struct SparseVec<T> {
    inner: Vec<Option<T>>,
}

impl<T> SparseVec<T> {
    pub const fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(index).and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.inner.get_mut(index).and_then(Option::as_mut)
    }

    pub fn push(&mut self, v: T) -> usize {
        let index;
        if let Some((_index, spot)) = self
            .inner
            .iter_mut()
            .enumerate()
            .find(|(_, spot)| spot.is_none())
        {
            index = _index;
            *spot = Some(v);
        } else {
            index = self.inner.len();
            self.inner.push(Some(v));
        }

        index
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        self.inner.get_mut(index).and_then(Option::take)
    }

    pub fn replace(&mut self, index: usize, v: T) -> Option<T> {
        // TODO: max file descriptor,
        // the user app can simply use a fd of 100000000000000 to crash the kernel
        self.inner
            .resize_with(self.inner.len().max(index + 1), || None);

        let slot = self.inner.get_mut(index).unwrap();

        let old = slot.take();
        *slot = Some(v);
        old
    }
}

//

// TODO: should be the same as the VFS FileDevice trait
pub trait FileDescriptor: Send + Sync {
    fn as_any(&self) -> &dyn Any;

    /// `end - start`
    fn len(&self) -> Result<usize> {
        Err(Error::INVALID_ARGUMENT)
    }

    fn is_empty(&self) -> Result<bool> {
        self.len().map(|len| len == 0)
    }

    /// truncate/add zeros
    #[allow(unused_variables)]
    fn set_len(&self, len: usize) -> Result<()> {
        Err(Error::INVALID_ARGUMENT)
    }

    // /// get the current read/write position
    // fn tell(&self) -> Result<usize> {
    //     Err(Error::INVALID_ARGUMENT)
    // }

    /// set the current read/write position
    #[allow(unused_variables)]
    fn seek(&self, offset: isize, origin: Seek) -> Result<usize> {
        Err(Error::INVALID_ARGUMENT)
    }

    /// read and advance the read/write position
    #[allow(unused_variables)]
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        Err(Error::INVALID_ARGUMENT)
    }

    /// write and advance the read/write position
    #[allow(unused_variables)]
    fn write(&self, buf: &[u8]) -> Result<usize> {
        Err(Error::INVALID_ARGUMENT)
    }
}

/// file descriptor backend that points to an opened VFS file
pub struct FileDescData {
    /// VFS node
    pub file_ref: FileRef<Futex>,

    /// the current read/write offset
    pub position: AtomicUsize,
}

impl FileDescData {
    pub const fn new(file_ref: FileRef<Futex>, position: usize) -> Self {
        Self {
            file_ref,
            position: AtomicUsize::new(position),
        }
    }

    pub fn open(path: &str) -> Result<Self> {
        VFS_ROOT
            .find_file(path, true, true)
            .map(Self::from)
            .map_err(map_vfs_err_to_syscall_err)
    }
}

impl Clone for FileDescData {
    fn clone(&self) -> Self {
        let position = AtomicUsize::new(self.position.load(Ordering::SeqCst));
        Self {
            file_ref: self.file_ref.clone(),
            position,
        }
    }
}

impl From<FileRef<Futex>> for FileDescData {
    fn from(file_ref: FileRef<Futex>) -> Self {
        Self {
            file_ref,
            position: AtomicUsize::new(0),
        }
    }
}

impl FileDescriptor for FileDescData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> Result<usize> {
        Ok(self.file_ref.lock().len())
    }

    fn set_len(&self, len: usize) -> Result<()> {
        self.file_ref
            .lock()
            .set_len(len)
            .map_err(map_vfs_err_to_syscall_err)
    }

    fn seek(&self, offset: isize, origin: Seek) -> Result<usize> {
        let pos = match origin {
            Seek::SET => {
                let lock = self.file_ref.lock();
                let offset = offset.abs_diff(0);
                self.position.store(offset, Ordering::SeqCst);
                drop(lock);
                offset
            }
            Seek::CUR => match offset.signum() {
                1 => {
                    let lock = self.file_ref.lock();
                    let pos = self.position.fetch_add(offset as usize, Ordering::SeqCst);
                    drop(lock);
                    pos
                }
                0 => self.position.load(Ordering::SeqCst),
                -1 => {
                    let lock = self.file_ref.lock();
                    let pos = self
                        .position
                        .fetch_sub((-offset) as usize, Ordering::SeqCst);
                    drop(lock);
                    pos
                }
                _ => unreachable!(),
            },
            Seek::END => {
                let lock = self.file_ref.lock();
                let pos = (lock.len() as isize + offset) as usize;
                self.position.store(pos, Ordering::SeqCst);
                drop(lock);
                pos
            }
            _ => return Err(Error::INVALID_FLAGS),
        };

        Ok(pos)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let lock = self.file_ref.lock();
        let bytes = lock
            .read(self.position.load(Ordering::SeqCst), buf)
            .map_err(map_vfs_err_to_syscall_err)?;
        self.position.fetch_add(bytes, Ordering::SeqCst);
        drop(lock);
        Ok(bytes)
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        let mut lock = self.file_ref.lock();
        let bytes = lock
            .write(self.position.load(Ordering::SeqCst), buf)
            .map_err(map_vfs_err_to_syscall_err)?;
        self.position.fetch_add(bytes, Ordering::SeqCst);
        drop(lock);
        Ok(bytes)
    }
}

impl FileDescriptor for Sender<u8> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> Result<usize> {
        Err(Error::IS_A_PIPE)
    }

    fn set_len(&self, _: usize) -> Result<()> {
        Err(Error::IS_A_PIPE)
    }

    fn seek(&self, _: isize, _: Seek) -> Result<usize> {
        Err(Error::IS_A_PIPE)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        if let Ok(n) = self.weak_recv_slice(buf) {
            Ok(n)
        } else {
            Ok(0)
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        if self.send_slice(buf).is_ok() {
            Ok(buf.len())
        } else {
            Ok(0)
        }
    }
}

impl FileDescriptor for Receiver<u8> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> Result<usize> {
        Err(Error::IS_A_PIPE)
    }

    fn set_len(&self, _: usize) -> Result<()> {
        Err(Error::IS_A_PIPE)
    }

    fn seek(&self, _: isize, _: Seek) -> Result<usize> {
        Err(Error::IS_A_PIPE)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        if let Ok(n) = self.recv_slice(buf) {
            Ok(n)
        } else {
            Ok(0)
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        if self.weak_send_slice(buf).is_ok() {
            Ok(buf.len())
        } else {
            Ok(0)
        }
    }
}

/// general socket backend info
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketInfo {
    pub domain: SocketDomain,
    pub ty: SocketType,
    pub proto: Protocol,
}

/// file descriptor backend that points to a local domain socket listener
pub struct LocalSocket {
    pub info: SocketInfo,
    pub inner: Once<LocalSocketType>,
}

pub enum LocalSocketType {
    Listener { incoming: Channel<SocketPipe> },
    Connection { pipe: SocketPipe },
    None,
}

impl LocalSocket {
    pub const fn new(info: SocketInfo) -> Self {
        Self {
            info,
            inner: Once::new(),
        }
    }

    pub const fn connected(info: SocketInfo, pipe: SocketPipe) -> Self {
        Self {
            info,
            inner: Once::initialized(LocalSocketType::Connection { pipe }),
        }
    }

    pub fn is_uninit(&self) -> bool {
        !self.inner.is_completed()
    }

    pub fn listener(&self) -> Result<&Channel<SocketPipe>> {
        let inner = self.inner.call_once(|| LocalSocketType::Listener {
            incoming: Channel::new(16),
        });

        if let LocalSocketType::Listener { incoming } = inner {
            Ok(incoming)
        } else {
            Err(Error::INVALID_ARGUMENT)
        }
    }

    pub fn connection(&self, pipe: SocketPipe) -> Result<&SocketPipe> {
        let inner = self
            .inner
            .call_once(move || LocalSocketType::Connection { pipe });

        if let LocalSocketType::Connection { pipe } = inner {
            Ok(pipe)
        } else {
            Err(Error::INVALID_ARGUMENT)
        }
    }

    fn inner(&self) -> &LocalSocketType {
        self.inner.get().unwrap_or(&LocalSocketType::None)
    }
}

impl FileDescriptor for LocalSocket {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> Result<usize> {
        Err(Error::IS_A_PIPE)
    }

    fn set_len(&self, _: usize) -> Result<()> {
        Err(Error::IS_A_PIPE)
    }

    fn seek(&self, _: isize, _: Seek) -> Result<usize> {
        Err(Error::IS_A_PIPE)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        match self.inner() {
            LocalSocketType::Connection { pipe } => Ok(pipe.recv.recv_slice(buf).unwrap_or(0)),
            _ => Err(Error::INVALID_ARGUMENT),
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        match self.inner() {
            LocalSocketType::Connection { pipe } => {
                Ok(pipe.send.send_slice(buf).map(|_| buf.len()).unwrap_or(0))
            }
            _ => Err(Error::INVALID_ARGUMENT),
        }
    }
}

/// local domain socket "pipe"
pub struct SocketPipe {
    pub send: Sender<u8>,
    pub recv: Receiver<u8>,
}

impl SocketPipe {
    pub fn new() -> (Self, Self) {
        let (send_0, recv_1) = pipe_with(0x1000).split();
        let (send_1, recv_0) = pipe_with(0x1000).split();
        (
            Self {
                send: send_0,
                recv: recv_0,
            },
            Self {
                send: send_1,
                recv: recv_1,
            },
        )
    }
}

/// VFS server socket file
pub struct BoundSocket(pub Arc<LocalSocket>);

impl FileDevice for BoundSocket {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn len(&self) -> usize {
        0
    }

    fn set_len(&mut self, _: usize) -> hyperion_vfs::error::IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    fn read(&self, _: usize, _: &mut [u8]) -> hyperion_vfs::error::IoResult<usize> {
        Err(IoError::PermissionDenied)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> hyperion_vfs::error::IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

//

pub struct ProcessExtra {
    pub files: Mutex<SparseVec<Arc<dyn FileDescriptor>>>,
    pub on_close: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
    pub cmdline: Once<ArcStr>,
}

impl Clone for ProcessExtra {
    fn clone(&self) -> Self {
        Self {
            files: Mutex::new(self.files.lock().clone()),
            on_close: Mutex::new(Vec::new()),
            cmdline: Once::new(),
        }
    }
}

impl ProcessExt for ProcessExtra {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn close(&self) {
        // FIXME: called twice with multiple threads + exit
        self.files.lock().inner.clear();
        /* for (i, fd) in self
            .files
            .lock()
            .inner
            .drain(..)
            .enumerate()
            .flat_map(|(i, s)| Some((i, s?)))
        {
            if Arc::strong_count(&fd) == 1 {
                hyperion_log::debug!("fd:{i} actually closed");
            } else {
                hyperion_log::debug!("fd:{i} closed (shared)");
            }
        } */
        for f in self.on_close.lock().drain(..) {
            f();
        }
    }
}

impl Drop for ProcessExtra {
    fn drop(&mut self) {
        self.close();
    }
}

//

pub fn exec(
    program: String,
    args: Vec<String>,
    stdin: Arc<dyn FileDescriptor>,
    stdout: Arc<dyn FileDescriptor>,
    stderr: Arc<dyn FileDescriptor>,
    on_close: Option<Box<dyn FnOnce() + Send>>,
) -> Pid {
    hyperion_scheduler::schedule(move || {
        // set its name
        hyperion_scheduler::rename(program.as_str());

        // set up /proc/self/cmdline
        let cmdline = slice::from_ref(&program).iter().chain(args.iter()).fold(
            String::new(),
            |mut acc, s| {
                acc.push_str(s);
                // cli args are null terminated + null separated (for compatibility)
                acc.push_str("\x00");
                acc
            },
        );
        hyperion_log::info!(" - cmdline: `{cmdline}`");
        with_proc_ext(move |ext| {
            ext.cmdline.call_once(move || cmdline.into());
        });

        // setup the STDIO
        fd_replace(FileDesc(0), stdin);
        fd_replace(FileDesc(1), stdout);
        fd_replace(FileDesc(2), stderr);
        if let Some(on_close) = on_close {
            crate::on_close(on_close);
        }

        // read the ELF file contents
        // FIXME: read before schedule and return any read errors
        let mut elf = Vec::new();
        let bin = VFS_ROOT
            .find_file(program.as_str(), false, false)
            .unwrap_or_else(|err| panic!("could not load ELF `{program}`: {err}"));
        let bin = bin.lock_arc();
        loop {
            let mut buf = [0; 64];
            let len = bin.read(elf.len(), &mut buf).unwrap();
            elf.extend_from_slice(&buf[..len]);
            if len == 0 {
                break;
            }
        }
        drop(bin);

        // load ..
        let loader = Loader::new(elf.as_ref());
        loader.load();
        let entry = loader.finish();

        // the elf is trying to steal our memory, drop the elf as a revenge
        drop(elf);

        // .. and exec the binary
        match entry {
            Ok(entry) => entry.enter(program, args),
            Err(_) => {
                error!("no ELF entrypoint");
                let stderr = fd_query(FileDesc(2)).unwrap();
                stderr.write(b"invalid ELF: entry point missing").unwrap();
            }
        }
    })
}

pub fn on_close(on_close: Box<dyn FnOnce() + Send>) {
    with_proc_ext(|ext| {
        ext.on_close.lock().push(on_close);
    });
}

pub fn fd_query(fd: FileDesc) -> Result<Arc<dyn FileDescriptor>> {
    with_proc_ext(|ext| {
        ext.files
            .lock()
            .get(fd.0)
            .ok_or(Error::BAD_FILE_DESCRIPTOR)
            .cloned()
    })
}

pub fn fd_query_of<T: FileDescriptor + Any + 'static>(fd: FileDesc) -> Result<Arc<T>> {
    let d = fd_query(fd)?;

    if d.as_any().is::<T>() {
        let ptr = Arc::into_raw(d) as *const T;
        // SAFETY: Arc downcast is safe, right??
        // it's going to be an enum later anyways
        Ok(unsafe { Arc::from_raw(ptr) })
    } else {
        Err(Error::INVALID_ARGUMENT)
    }

    // let s = Arc::new(4) as Arc<dyn Any>;
    // let s = s.downcast_ref::<i32>();

    // d.as_any().downcast_ref::<T>();
}

pub fn fd_push(data: Arc<dyn FileDescriptor>) -> FileDesc {
    with_proc_ext(|ext| FileDesc(ext.files.lock().push(data)))
}

pub fn fd_replace(fd: FileDesc, data: Arc<dyn FileDescriptor>) -> Option<Arc<dyn FileDescriptor>> {
    with_proc_ext(|ext| ext.files.lock().replace(fd.0, data))
}

pub fn fd_take(fd: FileDesc) -> Option<Arc<dyn FileDescriptor>> {
    with_proc_ext(|ext| ext.files.lock().remove(fd.0))
}

pub fn fd_copy(old: FileDesc, new: FileDesc) {
    with_proc_ext(|ext| {
        let mut files = ext.files.lock();

        if let Some(old) = files.get(old.0).cloned() {
            files.replace(new.0, old);
        }
    })
}

pub fn fd_clone_all() -> SparseVec<Arc<dyn FileDescriptor>> {
    with_proc_ext(|ext| ext.files.lock().clone())
}

pub fn with_proc_ext<F: FnOnce(&ProcessExtra) -> T, T>(f: F) -> T {
    let this = process();
    let ext = process_ext_with(&this);
    f(ext)
}

pub fn process_ext_with(proc: &Process) -> &ProcessExtra {
    proc.ext
        .call_once(|| {
            Box::new(ProcessExtra {
                files: Mutex::new(SparseVec::new()),
                on_close: Mutex::new(Vec::new()),
                cmdline: Once::new(),
            })
        })
        .as_any()
        .downcast_ref()
        .unwrap()
}

pub fn map_vfs_err_to_syscall_err(err: IoError) -> Error {
    match err {
        IoError::NotFound => Error::NOT_FOUND,
        IoError::AlreadyExists => Error::ALREADY_EXISTS,
        IoError::NotADirectory => Error::NOT_A_DIRECTORY,
        IoError::IsADirectory => Error::NOT_A_FILE,
        IoError::FilesystemError => Error::FILESYSTEM_ERROR,
        IoError::PermissionDenied => Error::PERMISSION_DENIED,
        IoError::UnexpectedEOF => Error::UNEXPECTED_EOF,
        IoError::Interrupted => Error::INTERRUPTED,
        IoError::WriteZero => Error::WRITE_ZERO,
    }
}

pub fn read_slice_parts(ptr: u64, len: u64) -> Result<(VirtAddr, usize)> {
    if len == 0 {
        return Ok((VirtAddr::new_truncate(0), 0));
    }

    let Some(end) = ptr.checked_add(len) else {
        return Err(Error::INVALID_ADDRESS);
    };

    let (Ok(start), Ok(end)) = (VirtAddr::try_new(ptr), VirtAddr::try_new(end)) else {
        return Err(Error::INVALID_ADDRESS);
    };

    if !process()
        .address_space
        .page_map
        .is_mapped(start..end, PageTableFlags::USER_ACCESSIBLE)
    {
        // debug!("{:?} not mapped", start..end);
        return Err(Error::INVALID_ADDRESS);
    }

    Ok((start, len as _))
}

pub fn read_untrusted_ref<'a, T>(ptr: u64) -> Result<&'a T> {
    if !(ptr as *const T).is_aligned() {
        return Err(Error::INVALID_ADDRESS);
    }

    read_slice_parts(ptr, mem::size_of::<T>() as _).map(|(start, _)| unsafe { &*start.as_ptr() })
}

pub fn read_untrusted_mut<'a, T>(ptr: u64) -> Result<&'a mut T> {
    if !(ptr as *const T).is_aligned() {
        hyperion_log::debug!("not aligned");
        return Err(Error::INVALID_ADDRESS);
    }

    read_slice_parts(ptr, mem::size_of::<T>() as _)
        .map(|(start, _)| unsafe { &mut *start.as_mut_ptr() })
}

pub fn read_untrusted_slice<'a, T: Copy>(ptr: u64, len: u64) -> Result<&'a [T]> {
    read_slice_parts(ptr, len).map(|(start, len)| {
        // TODO:
        // SAFETY: this is most likely unsafe
        if len == 0 {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(start.as_ptr(), len as _) }
        }
    })
}

pub fn read_untrusted_bytes<'a>(ptr: u64, len: u64) -> Result<&'a [u8]> {
    read_slice_parts(ptr, len).map(|(start, len)| {
        // TODO:
        // SAFETY: this is most likely unsafe
        if len == 0 {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(start.as_ptr(), len as _) }
        }
    })
}

pub fn read_untrusted_bytes_mut<'a>(ptr: u64, len: u64) -> Result<&'a mut [u8]> {
    read_slice_parts(ptr, len).map(|(start, len)| {
        // TODO:
        // SAFETY: this is most likely unsafe
        if len == 0 {
            &mut []
        } else {
            unsafe { core::slice::from_raw_parts_mut(start.as_mut_ptr(), len as _) }
        }
    })
}

pub fn read_untrusted_str<'a>(ptr: u64, len: u64) -> Result<&'a str> {
    read_untrusted_bytes(ptr, len)
        .and_then(|bytes| core::str::from_utf8(bytes).map_err(|_| Error::INVALID_UTF8))
}
