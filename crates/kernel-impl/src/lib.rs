#![no_std]
#![feature(pointer_is_aligned)]

//

extern crate alloc;

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::{
    any::Any,
    mem,
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperion_arch::vmm::PageMap;
use hyperion_mem::vmm::PageMapImpl;
use hyperion_scheduler::{
    ipc::pipe::{channel_with, Channel, Pipe, Receiver, Sender},
    lock::{Futex, Mutex},
    process,
    task::{Process, ProcessExt},
};
use hyperion_syscall::{
    err::{Error, Result},
    fs::{FileDesc, Seek},
    net::{Protocol, SocketDesc, SocketDomain, SocketType},
};
use hyperion_vfs::{
    device::FileDevice,
    error::{IoError, IoResult},
    tree::{FileRef, Node},
};
use lock_api::ArcMutexGuard;
use spin::Lazy;
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub static VFS_ROOT: Lazy<Node<Futex>> = Lazy::new(Node::new_root);

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

        hyperion_log::debug!("len:{} index:{}", self.inner.len(), index);

        let slot = self.inner.get_mut(index).unwrap();

        let old = slot.take();
        *slot = Some(v);
        old
    }
}

//

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
                let _lock = self.file_ref.lock();
                let offset = offset.abs_diff(0);
                self.position.store(offset, Ordering::SeqCst);
                offset
            }
            Seek::CUR => {
                if offset == 0 {
                    self.position.load(Ordering::SeqCst)
                } else if offset > 0 {
                    let _lock = self.file_ref.lock();
                    self.position.fetch_add(offset as usize, Ordering::SeqCst)
                } else {
                    let _lock = self.file_ref.lock();
                    self.position
                        .fetch_sub((-offset) as usize, Ordering::SeqCst)
                }
            }
            Seek::END => {
                let lock = self.file_ref.lock();
                let pos = (lock.len() as isize + offset) as usize;
                self.position.store(pos, Ordering::SeqCst);
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
        Ok(bytes)
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        let lock = self.file_ref.lock();
        let bytes = self
            .file_ref
            .lock()
            .write(self.position.load(Ordering::SeqCst), buf)
            .map_err(map_vfs_err_to_syscall_err)?;
        self.position.fetch_add(bytes, Ordering::SeqCst);
        Ok(bytes)
    }
}

#[derive(Clone)]
pub struct PipeDescData {
    pipe: Arc<Pipe>,
}

impl FileDescriptor for PipeDescData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> Result<usize> {
        Err(Error::IS_A_PIPE)
    }

    fn set_len(&self, len: usize) -> Result<()> {
        Err(Error::IS_A_PIPE)
    }

    fn seek(&self, offset: isize, origin: Seek) -> Result<usize> {
        Err(Error::IS_A_PIPE)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        if let Ok(n) = self.pipe.recv_slice(buf) {
            Ok(n)
        } else {
            Ok(0)
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        if self.pipe.send_slice(buf).is_ok() {
            Ok(buf.len())
        } else {
            Ok(0)
        }
    }
}

/// general socket backend info
#[derive(Clone)]
pub struct SocketInfo {
    pub domain: SocketDomain,
    pub ty: SocketType,
    pub proto: Protocol,
}

/// file descriptor backend that points to a local domain socket listener
#[derive(Clone)]
pub struct SocketLocalListenerDescData {
    pub info: SocketInfo,
    pub incoming: Arc<Channel<SocketPipe>>,
}

impl SocketLocalListenerDescData {
    pub fn new(info: SocketInfo) -> Self {
        Self {
            info,
            incoming: Arc::new(Channel::new(16)),
        }
    }
}

impl FileDescriptor for SocketLocalListenerDescData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// file descriptor backend that points to a local domain socket connection
#[derive(Clone)]
pub struct SocketLocalConnDescData {
    pub info: SocketInfo,
    pub conn: SocketPipe,
}

impl FileDescriptor for SocketLocalConnDescData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> Result<usize> {
        Err(Error::IS_A_PIPE)
    }

    fn set_len(&self, len: usize) -> Result<()> {
        Err(Error::IS_A_PIPE)
    }

    fn seek(&self, offset: isize, origin: Seek) -> Result<usize> {
        Err(Error::IS_A_PIPE)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        if let Ok(n) = self.conn.recv.recv_slice(buf) {
            Ok(n)
        } else {
            Ok(0)
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        if self.conn.send.send_slice(buf).is_ok() {
            Ok(buf.len())
        } else {
            Ok(0)
        }
    }
}

/// local domain socket "pipe"
#[derive(Clone)]
pub struct SocketPipe {
    pub send: Sender<u8>,
    pub recv: Receiver<u8>,
}

impl SocketPipe {
    pub fn new() -> (Self, Self) {
        let (send_0, recv_1) = channel_with(0x1000);
        let (send_1, recv_0) = channel_with(0x1000);
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

//

pub struct ProcessExtra {
    pub files: Mutex<SparseVec<Arc<dyn FileDescriptor>>>,
}

impl Clone for ProcessExtra {
    fn clone(&self) -> Self {
        let files = Mutex::new(self.files.lock().clone());
        Self { files }
    }
}

impl ProcessExt for ProcessExtra {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn close(&self) {
        self.files.lock().inner.clear();
    }
}

//

pub fn fd_query(fd: FileDesc) -> Result<Arc<dyn FileDescriptor>> {
    with_proc_ext(|ext| {
        ext.files
            .lock()
            .get(fd.0)
            .ok_or(Error::BAD_FILE_DESCRIPTOR)
            .cloned()
    })
}

pub fn fd_push(data: Arc<dyn FileDescriptor>) -> FileDesc {
    with_proc_ext(|ext| FileDesc(ext.files.lock().push(data.into())))
}

pub fn fd_replace(fd: FileDesc, data: Arc<dyn FileDescriptor>) -> Option<Arc<dyn FileDescriptor>> {
    with_proc_ext(|ext| ext.files.lock().replace(fd.0, data.into()))
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

    if !PageMap::current().is_mapped(start..end, PageTableFlags::USER_ACCESSIBLE) {
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
