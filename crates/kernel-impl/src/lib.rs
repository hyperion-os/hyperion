#![no_std]
#![feature(pointer_is_aligned)]

//

extern crate alloc;

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::{any::Any, mem};

use hyperion_arch::vmm::PageMap;
use hyperion_mem::vmm::PageMapImpl;
use hyperion_scheduler::{
    ipc::pipe::{channel_with, Channel, Receiver, Sender},
    lock::{Futex, Mutex},
    process,
    task::{Process, ProcessExt},
};
use hyperion_syscall::{
    err::{Error, Result},
    fs::FileDesc,
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

pub struct PipeInput(pub Sender<u8>);

impl FileDevice for PipeInput {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        0
    }

    fn read(&self, _: usize, buf: &mut [u8]) -> IoResult<usize> {
        if let Ok(n) = self.0.weak_recv_slice(buf) {
            Ok(n)
        } else {
            Ok(0)
        }
    }

    fn write(&mut self, _: usize, data: &[u8]) -> IoResult<usize> {
        if self.0.send_slice(data).is_err() {
            Ok(0)
        } else {
            Ok(data.len())
        }
    }
}

pub struct PipeOutput(pub Receiver<u8>);

impl FileDevice for PipeOutput {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        0
    }

    fn read(&self, _: usize, buf: &mut [u8]) -> IoResult<usize> {
        if let Ok(n) = self.0.recv_slice(buf) {
            Ok(n)
        } else {
            Ok(0)
        }
    }

    fn write(&mut self, _: usize, data: &[u8]) -> IoResult<usize> {
        if self.0.weak_send_slice(data).is_err() {
            Ok(0)
        } else {
            Ok(data.len())
        }
    }
}

pub struct ProcessExtra {
    pub files: Mutex<SparseVec<File>>,
    pub sockets: Mutex<SparseVec<Socket>>,
}

pub type File = Arc<Mutex<FileInner>>;

pub struct FileInner {
    pub file_ref: FileRef<Futex>,
    pub position: usize,
}

pub struct Socket {
    pub socket_ref: SocketRef,
}

pub type SocketRef = Arc<Mutex<SocketFile>>;

pub struct SocketFile {
    pub domain: SocketDomain,
    pub ty: SocketType,
    pub proto: Protocol,

    pub incoming: Option<Arc<Channel<LocalSocketConn>>>,
    pub connection: Option<LocalSocketConn>,
}

impl SocketFile {
    pub fn incoming(&mut self) -> Arc<Channel<LocalSocketConn>> {
        self.incoming
            .get_or_insert_with(|| Arc::new(Channel::new(16)))
            .clone()
    }

    pub fn try_incoming(&self) -> Option<Arc<Channel<LocalSocketConn>>> {
        self.incoming.as_ref().cloned()
    }

    pub fn try_connection(&self) -> Option<LocalSocketConn> {
        self.connection.as_ref().cloned()
    }
}

#[derive(Clone)]
pub struct LocalSocketConn {
    pub send: Sender<u8>,
    pub recv: Receiver<u8>,
}

impl LocalSocketConn {
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

impl FileDevice for SocketFile {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        0
    }

    fn read(&self, _offset: usize, _buf: &mut [u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }

    fn write(&mut self, _offset: usize, _buf: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

impl ProcessExt for ProcessExtra {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

//

pub fn get_socket_locked(socket: SocketDesc) -> Result<ArcMutexGuard<Futex, SocketFile>> {
    get_socket(socket).map(|v| v.lock_arc())
}

pub fn get_socket(socket: SocketDesc) -> Result<Arc<Mutex<SocketFile>>> {
    let this = process();
    let ext = process_ext_with(&this);

    let socket = ext
        .sockets
        .lock()
        .get(socket.0)
        .ok_or(Error::BAD_FILE_DESCRIPTOR)?
        .socket_ref
        .clone();

    Ok(socket)
}

pub fn push_file(file: FileInner) -> FileDesc {
    let this = process();
    let ext = process_ext_with(&this);

    let file = File::new(Mutex::new(file));

    let fd = ext.files.lock().push(file);
    FileDesc(fd)
}

pub fn push_socket(socket: SocketFile) -> SocketDesc {
    let this = process();
    let ext = process_ext_with(&this);

    let socket = Socket {
        socket_ref: Arc::new(Mutex::new(socket)),
    };

    let fd = ext.sockets.lock().push(socket);
    SocketDesc(fd)
}

// fn get_file(file: FileDesc) -> Result {
//     let this = process();
//     let ext = process_ext_with(&this);

//     let mut files = ext.files.lock();

//     let file = files.get_mut(file.0).ok_or(Error::BAD_FILE_DESCRIPTOR)?;
// }

pub fn process_ext_with(proc: &Process) -> &ProcessExtra {
    proc.ext
        .call_once(|| {
            Box::new(ProcessExtra {
                files: Mutex::new(SparseVec::new()),
                sockets: Mutex::new(SparseVec::new()),
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
