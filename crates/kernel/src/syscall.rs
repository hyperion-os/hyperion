use alloc::{boxed::Box, string::ToString, sync::Arc, vec::Vec};
use core::{
    any::{type_name_of_val, Any},
    sync::atomic::Ordering,
};

use hyperion_arch::{stack::USER_HEAP_TOP, syscall::SyscallRegs, vmm::PageMap};
use hyperion_drivers::acpi::hpet::HPET;
use hyperion_instant::Instant;
use hyperion_kernel_impl::VFS_ROOT;
use hyperion_log::*;
use hyperion_mem::{
    pmm::{self, PageFrame},
    vmm::PageMapImpl,
};
use hyperion_scheduler::{
    ipc::pipe::{Channel, Pipe},
    lock::{Futex, Mutex},
    process,
    task::{Process, ProcessExt},
};
use hyperion_syscall::{
    err::{Error, Result},
    fs::FileOpenFlags,
    id,
    net::{Protocol, SocketDesc, SocketDomain, SocketType},
};
use hyperion_vfs::{
    device::FileDevice,
    error::{IoError, IoResult},
    path::Path,
    tree::{FileRef, Node},
};
use time::Duration;
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub fn syscall(args: &mut SyscallRegs) {
    let id = args.syscall_id;
    let (result, name) = match id as usize {
        id::LOG => call_id(log, args),
        id::EXIT => call_id(exit, args),
        id::YIELD_NOW => call_id(yield_now, args),
        id::TIMESTAMP => call_id(timestamp, args),
        id::NANOSLEEP => call_id(nanosleep, args),
        id::NANOSLEEP_UNTIL => call_id(nanosleep_until, args),
        id::PTHREAD_SPAWN => call_id(pthread_spawn, args),
        id::PALLOC => call_id(palloc, args),
        id::PFREE => call_id(pfree, args),
        id::SEND => call_id(send, args),
        id::RECV => call_id(recv, args),
        id::RENAME => call_id(rename, args),

        id::OPEN => call_id(open, args),
        id::CLOSE => call_id(close, args),
        id::READ => call_id(read, args),
        id::WRITE => call_id(write, args),

        id::SOCKET => call_id(socket, args),
        id::BIND => call_id(bind, args),
        id::LISTEN => call_id(listen, args),
        id::ACCEPT => call_id(accept, args),
        id::CONNECT => call_id(connect, args),

        _ => {
            debug!("invalid syscall");
            hyperion_scheduler::stop();
        }
    };

    _ = (result, name);
    // if result < 0 {
    //     debug!("syscall `{name}` (id {id}) returned {result}",);
    // }
}

fn call_id(
    f: impl FnOnce(&mut SyscallRegs) -> Result<usize>,
    args: &mut SyscallRegs,
) -> (Result<usize>, &str) {
    let name = type_name_of_val(&f);

    // debug!(
    //     "{name}<{}>({}, {}, {}, {}, {})",
    //     args.syscall_id, args.arg0, args.arg1, args.arg2, args.arg3, args.arg4,
    // );

    let res = f(args);
    args.syscall_id = Error::encode(res) as u64;
    (res, name)
}

/// print a string to logs
///
/// # arguments
///  - `syscall_id` : 1
///  - `arg0` : _utf8 string address_
///  - `arg1` : _utf8 string length_
pub fn log(args: &mut SyscallRegs) -> Result<usize> {
    let str = read_untrusted_str(args.arg0, args.arg1)?;
    hyperion_log::print!("{str}");
    return Ok(0);
}

/// exit and kill the current process
///
/// # arguments
///  - `syscall_id` : 2
///  - `arg0` : _exit code_
pub fn exit(_args: &mut SyscallRegs) -> Result<usize> {
    // TODO: exit code
    hyperion_scheduler::stop();
}

/// give the processor back to the kernel temporarily
///
/// # arguments
///  - `syscall_id` : 3
pub fn yield_now(_args: &mut SyscallRegs) -> Result<usize> {
    hyperion_scheduler::yield_now();
    return Ok(0);
}

/// get the number of nanoseconds after boot
///
/// # arguments
///  - `syscall_id` : 4
///  - `arg0` : address of a 128 bit variable where to store the timestamp
pub fn timestamp(args: &mut SyscallRegs) -> Result<usize> {
    let nanos = HPET.nanos();

    let bytes = read_untrusted_bytes_mut(args.arg0, 16)?;
    bytes.copy_from_slice(&nanos.to_ne_bytes());

    return Ok(0);
}

/// sleep at least arg0 nanoseconds
///
/// # arguments
///  - `syscall_id` : 5
///  - `arg0` : lower 64 bits of the 128 bit duration TODO: address to a 128 bit variable
pub fn nanosleep(args: &mut SyscallRegs) -> Result<usize> {
    hyperion_scheduler::sleep(Duration::nanoseconds((args.arg0 as i64).max(0)));
    return Ok(0);
}

/// sleep at least until the nanosecond arg0 happens
///
/// # arguments
///  - `syscall_id` : 6
///  - `arg0` : lower 64 bits of the 128 bit timestamp TODO: address to a 128 bit variable
pub fn nanosleep_until(args: &mut SyscallRegs) -> Result<usize> {
    hyperion_scheduler::sleep_until(Instant::new(args.arg0 as u128));
    return Ok(0);
}

/// spawn a new thread
///
/// thread entry signature: `extern "C" fn thread_entry(stack_ptr: usize, arg1: usize) -> !`
///
/// # arguments
///  - `syscall_id` : 8
///  - `arg0` : the thread function pointer
///  - `arg1` : the thread function argument
pub fn pthread_spawn(args: &mut SyscallRegs) -> Result<usize> {
    hyperion_scheduler::spawn_userspace(args.arg0, args.arg1);
    return Ok(0);
}

/// allocate physical pages and map them to virtual memory
///
/// returns the virtual address pointer
///
/// # arguments
///  - `syscall_id` : 9
///  - `arg0` : page count
pub fn palloc(args: &mut SyscallRegs) -> Result<usize> {
    let pages = args.arg0 as usize;
    let alloc = pages * 0x1000;

    let active = hyperion_scheduler::process();
    let mut allocs = active.allocs.bitmap();
    let alloc_bottom = active.heap_bottom.fetch_add(alloc, Ordering::SeqCst);
    let alloc_top = alloc_bottom + alloc;

    if alloc_top as u64 >= USER_HEAP_TOP {
        return Err(Error::OUT_OF_VIRTUAL_MEMORY);
    }

    let frames = pmm::PFA.alloc(pages);
    active.address_space.page_map.map(
        VirtAddr::new(alloc_bottom as _)..VirtAddr::new(alloc_top as _),
        frames.physical_addr(),
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
    );

    let page_bottom = alloc_bottom / 0x1000;
    for page in page_bottom..page_bottom + pages {
        allocs.set(page, true).unwrap();
    }

    return Ok(alloc_bottom);
}

/// free allocated physical pages
///
/// # arguments
///  - `syscall_id` : 10
///  - `arg0` : page
///  - `arg1` : page count
pub fn pfree(args: &mut SyscallRegs) -> Result<usize> {
    let Ok(alloc_bottom) = VirtAddr::try_new(args.arg0) else {
        return Err(Error::INVALID_ADDRESS);
    };
    let pages = args.arg1 as usize;

    let active = hyperion_scheduler::process();
    let mut allocs = active.allocs.bitmap();

    let page_bottom = alloc_bottom.as_u64() as usize / 0x1000;
    for page in page_bottom..page_bottom + pages {
        if !allocs.get(page).unwrap() {
            return Err(Error::INVALID_ALLOC);
        }

        allocs.set(page, false).unwrap();
    }

    let Some(palloc) = active.address_space.page_map.virt_to_phys(alloc_bottom) else {
        return Err(Error::INVALID_ADDRESS);
    };

    let frames = unsafe { PageFrame::new(palloc, pages) };
    pmm::PFA.free(frames);
    active
        .address_space
        .page_map
        .unmap(alloc_bottom..alloc_bottom + pages * 0x1000);

    return Ok(0);
}

/// send data to an input channel of a process
///
/// # arguments
///  - `syscall_id` : 11
///  - `arg0`       : target PID
///  - `arg1`       : data ptr
///  - `arg2`       : data len (bytes)
pub fn send(args: &mut SyscallRegs) -> Result<usize> {
    let target_pid = args.arg0;
    let data = read_untrusted_bytes(args.arg1, args.arg2)?;

    let pid = hyperion_scheduler::task::Pid::new(target_pid as usize);

    if hyperion_scheduler::send(pid, data).is_err() {
        return Err(Error::NO_SUCH_PROCESS);
    }

    return Ok(0);
}

/// recv data from this process input channel
///
/// returns the number of bytes read
///
/// # arguments
///  - `syscall_id` : 12
///  - `arg0`       : data ptr
///  - `arg1`       : data len (bytes)
pub fn recv(args: &mut SyscallRegs) -> Result<usize> {
    let buf = read_untrusted_bytes_mut(args.arg0, args.arg1)?;
    return Ok(hyperion_scheduler::recv(buf));
}

/// rename the current process
///
/// # arguments
///  - `syscall_id` : 13
///  - `arg0` : filename : _utf8 string address_
///  - `arg1` : filename : _utf8 string length_
pub fn rename(args: &mut SyscallRegs) -> Result<usize> {
    let new_name = read_untrusted_str(args.arg0, args.arg1)?;
    hyperion_scheduler::rename(new_name.to_string().into());
    return Ok(0);
}

/// open a file
///
/// # arguments
///  - `syscall_id` : 1000
///  - `arg0` : filename : _utf8 string address_
///  - `arg1` : filename : _utf8 string length_
///  - `arg2` : flags
///  - `arg3` : mode
pub fn open(args: &mut SyscallRegs) -> Result<usize> {
    let path = read_untrusted_str(args.arg0, args.arg1)?;

    let Some(flags) = FileOpenFlags::from_bits(args.arg2 as usize) else {
        return Err(Error::INVALID_FLAGS);
    };

    let this = process();
    let ext = process_ext_with(&this);

    let create = flags.contains(FileOpenFlags::CREATE) || flags.contains(FileOpenFlags::CREATE_NEW);

    if flags.contains(FileOpenFlags::CREATE_NEW)
        || flags.contains(FileOpenFlags::TRUNC)
        || flags.contains(FileOpenFlags::APPEND)
        || (!flags.contains(FileOpenFlags::READ) && !flags.contains(FileOpenFlags::WRITE))
    {
        return Err(Error::FILESYSTEM_ERROR);
    }

    let mkdirs = true; // TODO: tmp

    let file_ref = VFS_ROOT
        .find_file(path, mkdirs, create)
        .map_err(map_vfs_err_to_syscall_err)?;
    let file = Some(File {
        file_ref,
        position: 0,
    });

    let mut files = ext.files.lock();

    let fd;
    if let Some((_fd, spot)) = files
        .iter_mut()
        .enumerate()
        .find(|(_, file)| file.is_none())
    {
        fd = _fd;
        *spot = file;
    } else {
        fd = files.len();
        files.push(file);
    }

    return Ok(fd);
}

/// close a file
///
/// # arguments
///  - `syscall_id` : 1100
///  - `arg0` : file descriptor
pub fn close(args: &mut SyscallRegs) -> Result<usize> {
    let this = process();
    let ext = process_ext_with(&this);

    *ext.files
        .lock()
        .get_mut(args.arg0 as usize)
        .ok_or(Error::BAD_FILE_DESCRIPTOR)? = None;

    return Ok(0);
}

/// read bytes from a file
///
/// # arguments
///  - `syscall_id` : 1200
///  - `arg0` : file descriptor
///  - `arg1` : data ptr
///  - `arg2` : data len (bytes)
///
/// # return values (syscall_id)
///  - `0`   : EOF
///  - `1..` : number of bytes read
pub fn read(args: &mut SyscallRegs) -> Result<usize> {
    let buf = read_untrusted_bytes_mut(args.arg1, args.arg2)?;

    let this = process();
    let ext = process_ext_with(&this);

    let mut files = ext.files.lock();

    let file = files
        .get_mut(args.arg0 as usize)
        .and_then(|v| v.as_mut())
        .ok_or(Error::BAD_FILE_DESCRIPTOR)?;

    let read = file
        .file_ref
        .lock()
        .read(file.position, buf)
        .map_err(map_vfs_err_to_syscall_err)?;
    file.position += read;

    return Ok(read);
}

/// write bytes into a file
///
/// # arguments
///  - `syscall_id` : 1300
///  - `arg0` : file descriptor
///  - `arg1` : data ptr
///  - `arg2` : data len (bytes)
///
/// # return values (syscall_id)
///  - `0..` : number of bytes written
pub fn write(args: &mut SyscallRegs) -> Result<usize> {
    let buf = read_untrusted_bytes(args.arg1, args.arg2)?;

    let this = process();
    let ext = process_ext_with(&this);

    let mut files = ext.files.lock();

    let file = files
        .get_mut(args.arg0 as usize)
        .and_then(|v| v.as_mut())
        .ok_or(Error::BAD_FILE_DESCRIPTOR)?;

    let written = file
        .file_ref
        .lock()
        .write(file.position, buf)
        .map_err(map_vfs_err_to_syscall_err)?;
    file.position += written;

    return Ok(written);
}

/// create a socket
///
/// [`hyperion_syscall::socket`]
fn socket(args: &mut SyscallRegs) -> Result<usize> {
    let domain = SocketDomain(args.arg0 as _);
    let ty = SocketType(args.arg1 as _);
    let proto = Protocol(args.arg2 as _);

    _socket(domain, ty, proto).map(|fd| fd.0)
}

fn _socket(domain: SocketDomain, ty: SocketType, proto: Protocol) -> Result<SocketDesc> {
    if domain != SocketDomain::LOCAL {
        return Err(Error::INVALID_DOMAIN);
    }

    if ty != SocketType::STREAM {
        return Err(Error::INVALID_TYPE);
    }

    if proto != Protocol::LOCAL {
        return Err(Error::UNKNOWN_PROTOCOL);
    }

    Ok(_socket_from(SocketFile {
        domain,
        ty,
        proto,
        conn: None,
        pipe: None,
    }))
}

fn _socket_from(socket: SocketFile) -> SocketDesc {
    let this = process();
    let ext = process_ext_with(&this);

    let socket = Some(Socket {
        socket_ref: Arc::new(Mutex::new(socket)),
    });

    let mut sockets = ext.sockets.lock();

    let fd;
    if let Some((_fd, spot)) = sockets
        .iter_mut()
        .enumerate()
        .find(|(_, socket)| socket.is_none())
    {
        fd = _fd;
        *spot = socket;
    } else {
        fd = sockets.len();
        sockets.push(socket);
    }

    return SocketDesc(fd);
}

/// bind a socket
///
/// [`hyperion_syscall::bind`]
fn bind(args: &mut SyscallRegs) -> Result<usize> {
    let socket = SocketDesc(args.arg0 as _);
    let addr = read_untrusted_str(args.arg1, args.arg2)?;

    _bind(socket, addr).map(|_| 0)
}

fn _bind(socket: SocketDesc, addr: &str) -> Result<()> {
    // TODO: this is only for LOCAL domain sockets atm
    let path = Path::from_str(addr);
    let Some((dir, sock_file)) = path.split() else {
        return Err(Error::NOT_FOUND);
    };

    let this = process();
    let ext = process_ext_with(&this);

    let sockets = ext.sockets.lock();
    let socket_file = sockets
        .get(socket.0)
        .and_then(|s| s.as_ref())
        .ok_or(Error::BAD_FILE_DESCRIPTOR)?
        .socket_ref
        .clone();
    drop(sockets);

    let dir = VFS_ROOT
        .find_dir(dir, false)
        .map_err(map_vfs_err_to_syscall_err)?;

    dir.lock()
        .create_node(sock_file, Node::File(socket_file))
        .map_err(map_vfs_err_to_syscall_err)?;

    return Ok(());
}

/// start listening to connections on a socket
///
/// [`hyperion_syscall::listen`]
fn listen(args: &mut SyscallRegs) -> Result<usize> {
    let socket = SocketDesc(args.arg0 as _);
    _listen(socket).map(|_| 0)
}

fn _listen(socket: SocketDesc) -> Result<()> {
    let this = process();
    let ext = process_ext_with(&this);

    ext.sockets
        .lock()
        .get(socket.0)
        .and_then(|s| s.as_ref())
        .ok_or(Error::BAD_FILE_DESCRIPTOR)?
        .socket_ref
        .lock()
        .conn = Some(Arc::new(Channel::new()));

    Ok(())
}

/// accept a connection on a socket
///
/// [`hyperion_syscall::accept`]
fn accept(args: &mut SyscallRegs) -> Result<usize> {
    let socket = SocketDesc(args.arg0 as _);

    _accept(socket).map(|fd| fd.0)
}

fn _accept(socket: SocketDesc) -> Result<SocketDesc> {
    let this = process();
    let ext = process_ext_with(&this);

    let sockets = ext.sockets.lock();
    let socket = sockets
        .get(socket.0)
        .and_then(|s| s.as_ref())
        .ok_or(Error::BAD_FILE_DESCRIPTOR)?
        .socket_ref
        .clone();
    drop(sockets);

    let mut socket = socket.lock();

    let domain = socket.domain;
    let ty = socket.ty;
    let proto = socket.proto;

    // `listen` syscall is not required
    let conn = socket
        .conn
        .get_or_insert_with(|| Arc::new(Channel::new()))
        .clone();

    drop(socket);

    // blocks here
    let pipe = conn.recv();

    Ok(_socket_from(SocketFile {
        domain,
        ty,
        proto,
        conn: None,
        pipe: Some(pipe),
    }))
}

/// connect to a socket
///
/// [`hyperion_syscall::connect`]
fn connect(args: &mut SyscallRegs) -> Result<usize> {
    let socket = SocketDesc(args.arg0 as _);
    let addr = read_untrusted_str(args.arg1, args.arg2)?;

    _connect(socket, addr).map(|_| 0)
}

fn _connect(socket: SocketDesc, addr: &str) -> Result<()> {
    let this = process();
    let ext = process_ext_with(&this);

    let sockets = ext.sockets.lock();
    let client = sockets
        .get(socket.0)
        .and_then(|s| s.as_ref())
        .ok_or(Error::BAD_FILE_DESCRIPTOR)?
        .socket_ref
        .clone();
    drop(sockets);

    let server = VFS_ROOT
        .find_file(addr, false, false)
        .map_err(map_vfs_err_to_syscall_err)?;
    let server = server.lock();

    // TODO: inode
    let conn = server
        .as_any()
        .downcast_ref::<SocketFile>()
        .ok_or(Error::CONNECTION_REFUSED)?
        .conn
        .as_ref()
        .cloned(); // not a socket file

    let Some(conn) = conn else {
        return Err(Error::CONNECTION_REFUSED);
    };

    drop(server);

    let pipe = Arc::new(Pipe::new());
    conn.send(pipe.clone());

    client.lock().pipe = Some(pipe);

    Ok(())
}

//

struct ProcessExtra {
    files: Mutex<Vec<Option<File>>>,
    sockets: Mutex<Vec<Option<Socket>>>,
}

struct File {
    file_ref: FileRef<Futex>,
    position: usize,
}

struct Socket {
    socket_ref: SocketRef,
}

type SocketRef = Arc<Mutex<SocketFile>>;

struct SocketFile {
    domain: SocketDomain,
    ty: SocketType,
    proto: Protocol,

    conn: Option<Arc<Channel<16, Arc<Pipe>>>>,
    pipe: Option<Arc<Pipe>>,
}

impl FileDevice for SocketFile {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        if let Some(pipe) = self.pipe.as_ref() {
            let recv = pipe.n_recv.load(Ordering::SeqCst);
            let send = pipe.n_send.load(Ordering::SeqCst);
            send - recv
        } else {
            0
        }
    }

    fn read(&self, _offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        let pipe = self.pipe.as_ref().ok_or(IoError::PermissionDenied)?;
        Ok(pipe.recv_slice(buf))
    }

    fn write(&mut self, _offset: usize, buf: &[u8]) -> IoResult<usize> {
        let pipe = self.pipe.as_ref().ok_or(IoError::PermissionDenied)?;
        pipe.send_slice(buf);
        Ok(buf.len())
    }
}

impl ProcessExt for ProcessExtra {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

//

fn process_ext_with(proc: &Process) -> &ProcessExtra {
    proc.ext
        .call_once(|| {
            Box::new(ProcessExtra {
                files: Mutex::new(Vec::new()),
                sockets: Mutex::new(Vec::new()),
            })
        })
        .as_any()
        .downcast_ref()
        .unwrap()
}

fn map_vfs_err_to_syscall_err(err: IoError) -> Error {
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

fn read_slice_parts(ptr: u64, len: u64) -> Result<(VirtAddr, usize)> {
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

fn read_untrusted_bytes<'a>(ptr: u64, len: u64) -> Result<&'a [u8]> {
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

fn read_untrusted_bytes_mut<'a>(ptr: u64, len: u64) -> Result<&'a mut [u8]> {
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

fn read_untrusted_str<'a>(ptr: u64, len: u64) -> Result<&'a str> {
    read_untrusted_bytes(ptr, len)
        .and_then(|bytes| core::str::from_utf8(bytes).map_err(|_| Error::INVALID_UTF8))
}
