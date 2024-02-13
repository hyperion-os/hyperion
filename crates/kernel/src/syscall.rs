use alloc::{
    boxed::Box,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::{
    fmt::Write,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperion_arch::syscall::SyscallRegs;
use hyperion_defer::DeferInit;
use hyperion_drivers::acpi::hpet::HPET;
use hyperion_instant::Instant;
use hyperion_kernel_impl::{
    fd_push, fd_query, fd_query_of, fd_replace, fd_take, map_vfs_err_to_syscall_err,
    read_untrusted_bytes, read_untrusted_bytes_mut, read_untrusted_mut, read_untrusted_ref,
    read_untrusted_slice, read_untrusted_str, BoundSocket, FileDescData, FileDescriptor,
    LocalSocket, SocketInfo, SocketPipe, VFS_ROOT,
};
use hyperion_loader::Loader;
use hyperion_log::*;
use hyperion_mem::{pmm::PageFrame, vmm::PageMapImpl};
use hyperion_scheduler::{
    futex,
    lock::{Lazy, Mutex},
    process, schedule, task,
    task::{AllocErr, FreeErr},
};
use hyperion_syscall::{
    err::{Error, Result},
    fs::{FileDesc, FileOpenFlags, Metadata, Seek},
    id,
    net::{Protocol, SocketDomain, SocketType},
};
use hyperion_vfs::{path::Path, ramdisk, tree::Node};
use time::Duration;
use x86_64::{align_down, align_up, structures::paging::PageTableFlags, VirtAddr};

//

pub fn syscall(args: &mut SyscallRegs) {
    // static SYSCALL_RESULTS: hyperion_futures::mpmc::Channel<()> =
    //     hyperion_futures::mpmc::Channel::new();

    // process syscall args

    // dispatch / run the syscall

    // hyperion_futures::spawn(async { SYSCALL_RESULTS.send(()) });

    // block on syscall futures

    // hyperion_futures::block_on(async {
    //     let task = SYSCALL_RESULTS.recv().await;
    //     debug!("recv: {task:?}");
    // });

    // return to the same or another task

    match args.syscall_id as usize {
        id::LOG => call_id(log, args),
        id::EXIT => call_id(exit, args),
        id::DONE => call_id(done, args),
        id::YIELD_NOW => call_id(yield_now, args),
        id::TIMESTAMP => call_id(timestamp, args),
        id::NANOSLEEP => call_id(nanosleep, args),
        id::NANOSLEEP_UNTIL => call_id(nanosleep_until, args),
        id::SPAWN => call_id(spawn, args),
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

        id::GET_PID => call_id(get_pid, args),
        id::GET_TID => call_id(get_tid, args),

        id::DUP => call_id(dup, args),
        id::FUTEX_WAIT => call_id(futex_wait, args),
        id::FUTEX_WAKE => call_id(futex_wake, args),

        id::MAP_FILE => call_id(map_file, args),
        id::UNMAP_FILE => call_id(unmap_file, args),
        id::METADATA => call_id(metadata, args),
        id::SEEK => call_id(seek, args),

        id::SYSTEM => call_id(system, args),

        other => {
            debug!("invalid syscall ({other})");
            hyperion_scheduler::exit();
        }
    };
}

fn call_id(f: impl FnOnce(&mut SyscallRegs) -> Result<usize>, args: &mut SyscallRegs) {
    // debug!("{}", core::any::type_name_of_val(&f));

    // debug!(
    //     "{}<{}>({}, {}, {}, {}, {})",
    //     core::any::type_name_of_val(&f),
    //     args.syscall_id,
    //     args.arg0,
    //     args.arg1,
    //     args.arg2,
    //     args.arg3,
    //     args.arg4,
    // );

    let res = f(args);
    // debug!(" \\= {res:?}");
    args.syscall_id = Error::encode(res) as u64;
}

/// print a string to logs
///
/// [`hyperion_syscall::log`]
pub fn log(args: &mut SyscallRegs) -> Result<usize> {
    let str = read_untrusted_str(args.arg0, args.arg1)?;

    _log(str);
    return Ok(0);
}

// #[trace]
fn _log(str: &str) {
    print!("{str}");
}

/// exit and kill the current process
///
/// [`hyperion_syscall::exit`]
pub fn exit(_args: &mut SyscallRegs) -> Result<usize> {
    _exit();
    return Ok(0);
}

// #[trace(split)]
fn _exit() {
    // TODO: exit code
    hyperion_scheduler::exit();
}

/// exit and kill the current thread
///
/// [`hyperion_syscall::done`]
pub fn done(_args: &mut SyscallRegs) -> Result<usize> {
    _done();
    return Ok(0);
}

// #[trace(split)]
fn _done() {
    // TODO: exit code
    hyperion_scheduler::done();
}

/// give the processor back to the kernel temporarily
///
/// [`hyperion_syscall::yield_now`]
pub fn yield_now(_args: &mut SyscallRegs) -> Result<usize> {
    _yield_now();
    return Ok(0);
}

// #[trace]
fn _yield_now() {
    hyperion_scheduler::yield_now();
}

/// get the number of nanoseconds after boot
///
/// [`hyperion_syscall::timestamp`]
pub fn timestamp(args: &mut SyscallRegs) -> Result<usize> {
    let bytes = read_untrusted_bytes_mut(args.arg0, 16)?;

    _timestamp(bytes);
    return Ok(0);
}

// #[trace]
fn _timestamp(bytes: &mut [u8]) {
    let nanos = HPET.nanos();
    bytes.copy_from_slice(&nanos.to_ne_bytes());
}

/// sleep at least arg0 nanoseconds
///
/// [`hyperion_syscall::nanosleep`]
pub fn nanosleep(args: &mut SyscallRegs) -> Result<usize> {
    let nanos = args.arg0 as i64;

    _nanosleep(nanos);
    return Ok(0);
}

// #[trace]
fn _nanosleep(nanos: i64) {
    hyperion_scheduler::sleep(Duration::nanoseconds(nanos.max(0)));
}

/// sleep at least until the nanosecond arg0 happens
///
/// [`hyperion_syscall::nanosleep_until`]
pub fn nanosleep_until(args: &mut SyscallRegs) -> Result<usize> {
    let nanosecond = args.arg0 as u128;

    _nanosleep_until(nanosecond);
    return Ok(0);
}

// #[trace]
fn _nanosleep_until(nanosecond: u128) {
    hyperion_scheduler::sleep_until(Instant::new(nanosecond));
}

/// spawn a new thread
///
/// thread entry signature: `extern "C" fn thread_entry(stack_ptr: usize, arg1: usize) -> !`
///
/// [`hyperion_syscall::spawn`]
pub fn spawn(args: &mut SyscallRegs) -> Result<usize> {
    _spawn(args.arg0, args.arg1);
    return Ok(0);
}

// #[trace]
fn _spawn(fn_ptr: u64, fn_arg: u64) {
    hyperion_scheduler::spawn_userspace(fn_ptr, fn_arg);
}

/// allocate physical pages and map them to virtual memory
///
/// returns the virtual address pointer
///
/// [`hyperion_syscall::palloc`]
pub fn palloc(args: &mut SyscallRegs) -> Result<usize> {
    let n_pages = args.arg0 as usize;
    return _palloc(n_pages).map(|ptr| ptr.as_u64() as _);
}

// #[trace]
fn _palloc(n_pages: usize) -> Result<VirtAddr> {
    let flags =
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;

    match process().alloc(n_pages, flags) {
        Ok((ptr, _)) => Ok(ptr),
        Err(AllocErr::OutOfVirtMem) => Err(Error::OUT_OF_VIRTUAL_MEMORY),
    }
}

/// free allocated physical pages
///
/// [`hyperion_syscall::pfree`]
pub fn pfree(args: &mut SyscallRegs) -> Result<usize> {
    let Ok(ptr) = VirtAddr::try_new(args.arg0) else {
        return Err(Error::INVALID_ADDRESS);
    };
    let n_pages = args.arg1 as usize;

    _pfree(ptr, n_pages)?;
    return Ok(0);
}

// #[trace]
fn _pfree(ptr: VirtAddr, n_pages: usize) -> Result<()> {
    match process().free(n_pages, ptr) {
        Ok(()) => Ok(()),
        Err(FreeErr::InvalidAddr) => Err(Error::INVALID_ADDRESS),
        Err(FreeErr::InvalidAlloc) => Err(Error::INVALID_ALLOC),
    }
}

/// rename the current process
///
/// [`hyperion_syscall::rename`]
pub fn rename(args: &mut SyscallRegs) -> Result<usize> {
    let new_name = read_untrusted_str(args.arg0, args.arg1)?;

    _rename(new_name);
    return Ok(0);
}

// #[trace]
fn _rename(new_name: &str) {
    hyperion_scheduler::rename(new_name.to_string());
}

/// open a file
///
/// [`hyperion_syscall::open`]
pub fn open(args: &mut SyscallRegs) -> Result<usize> {
    let path = read_untrusted_str(args.arg0, args.arg1)?;
    let Some(flags) = FileOpenFlags::from_bits(args.arg2 as usize) else {
        return Err(Error::INVALID_FLAGS);
    };

    let fd = _open(path, flags)?;
    return Ok(fd.0);
}

// #[trace]
fn _open(path: &str, flags: FileOpenFlags) -> Result<FileDesc> {
    let create = flags.contains(FileOpenFlags::CREATE) || flags.contains(FileOpenFlags::CREATE_NEW);
    let create_dirs = flags.contains(FileOpenFlags::CREATE_DIRS);

    if flags.contains(FileOpenFlags::IS_DIR) {
        _open_dir(path, flags, create, create_dirs)
    } else {
        _open_file(path, flags, create, create_dirs)
    }
}

fn _open_dir(
    path: &str,
    flags: FileOpenFlags,
    create: bool,
    create_dirs: bool,
) -> Result<FileDesc> {
    if flags.intersects(FileOpenFlags::WRITE | FileOpenFlags::TRUNC | FileOpenFlags::APPEND) {
        // tried to open a directory with write
        return Err(Error::INVALID_FLAGS);
    }

    let mut dir = VFS_ROOT
        .find_dir(path, create_dirs, create) // TODO: mkdir
        .map_err(map_vfs_err_to_syscall_err)?
        .lock_arc();

    let s = dir.nodes().map_err(map_vfs_err_to_syscall_err)?;

    let mut buf = String::new(); // TODO: real readdir
    for name in s.iter() {
        let node = dir.get_node(name).map_err(map_vfs_err_to_syscall_err)?;

        let (mode, size) = match node {
            Node::File(f) => ('f', f.lock().len()),
            Node::Directory(_) => ('d', 0),
        };

        writeln!(&mut buf, "{name} {size} {mode}").unwrap();
    }

    let fd = fd_push(Arc::new(FileDescData {
        file_ref: Arc::new(Mutex::new(ramdisk::File::new(buf.as_bytes()))),
        position: AtomicUsize::new(0),
    }));

    return Ok(fd);
}

fn _open_file(
    path: &str,
    flags: FileOpenFlags,
    create: bool,
    create_dirs: bool,
) -> Result<FileDesc> {
    if !flags.intersects(FileOpenFlags::READ | FileOpenFlags::WRITE) {
        // tried to open a file with no read and no write
        return Err(Error::INVALID_FLAGS);
    }

    let todo = FileOpenFlags::CREATE_NEW;
    if flags.intersects(todo) {
        error!("TODO `open` flags: {:?}", flags.intersection(todo));
        return Err(Error::FILESYSTEM_ERROR);
    }

    let file_ref = VFS_ROOT
        .find_file(path, create_dirs, create)
        .map_err(map_vfs_err_to_syscall_err)?;

    // let mut file_lock = file_ref.lock();
    let mut file_lock = DeferInit::new(|| file_ref.lock());
    if flags.contains(FileOpenFlags::TRUNC) {
        file_lock.set_len(0).map_err(map_vfs_err_to_syscall_err)?;
    }

    let position = if flags.contains(FileOpenFlags::APPEND) {
        file_lock.len()
    } else {
        0
    };

    drop(file_lock);

    return Ok(fd_push(Arc::new(FileDescData::new(file_ref, position))));
}

/// close a file
///
/// [`hyperion_syscall::close`]
pub fn close(args: &mut SyscallRegs) -> Result<usize> {
    let fd = FileDesc(args.arg0 as usize);

    _close(fd)?;
    return Ok(0);
}

// #[trace]
fn _close(fd: FileDesc) -> Result<()> {
    if fd_take(fd).is_none() {
        return Err(Error::BAD_FILE_DESCRIPTOR);
    }
    Ok(())
}

/// read bytes from a file
///
/// [`hyperion_syscall::read`]
pub fn read(args: &mut SyscallRegs) -> Result<usize> {
    let fd = FileDesc(args.arg0 as usize);
    let buf = read_untrusted_bytes_mut(args.arg1, args.arg2)?;

    return _read(fd, buf);
}

// #[trace]
fn _read(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    let file = fd_query(fd)?;
    return file.read(buf);
}

/// write bytes into a file
///
/// [`hyperion_syscall::write`]
pub fn write(args: &mut SyscallRegs) -> Result<usize> {
    let fd = FileDesc(args.arg0 as usize);
    let buf = read_untrusted_bytes(args.arg1, args.arg2)?;

    return _write(fd, buf);
}

fn _write(fd: FileDesc, buf: &[u8]) -> Result<usize> {
    let file = fd_query(fd)?;
    return file.write(buf);
}

/// create a socket
///
/// [`hyperion_syscall::socket`]
fn socket(args: &mut SyscallRegs) -> Result<usize> {
    let domain = SocketDomain(args.arg0 as _);
    let ty = SocketType(args.arg1 as _);
    let proto = Protocol(args.arg2 as _);
    let info = SocketInfo { domain, ty, proto };

    return _socket(info).map(|fd| fd.0);
}

fn _socket(info: SocketInfo) -> Result<FileDesc> {
    if info.domain != SocketDomain::LOCAL {
        return Err(Error::INVALID_DOMAIN);
    }

    if info.ty != SocketType::STREAM {
        return Err(Error::INVALID_TYPE);
    }

    if info.proto != Protocol::LOCAL {
        return Err(Error::UNKNOWN_PROTOCOL);
    }

    Ok(fd_push(Arc::new(LocalSocket::new(info))))
}

/// bind a socket
///
/// [`hyperion_syscall::bind`]
fn bind(args: &mut SyscallRegs) -> Result<usize> {
    let socket_fd = FileDesc(args.arg0 as _);
    let addr = read_untrusted_str(args.arg1, args.arg2)?;

    return _bind(socket_fd, addr).map(|_| 0);
}

fn _bind(socket_fd: FileDesc, addr: &str) -> Result<()> {
    // TODO: this is only for LOCAL domain sockets atm
    let path = Path::from_str(addr);
    let (dir, sock_file) = path.split();

    let socket = fd_query_of::<LocalSocket>(socket_fd)?;

    VFS_ROOT
        // find the directory node
        .find_dir(dir, false, true)
        .map_err(map_vfs_err_to_syscall_err)?
        // lock the directory
        .lock_arc()
        // create the socket file in that directory
        .create_node(
            sock_file,
            Node::File(Arc::new(Mutex::new(BoundSocket(socket)))),
        )
        .map_err(map_vfs_err_to_syscall_err)?;

    return Ok(());
}

/// start listening to connections on a socket
///
/// [`hyperion_syscall::listen`]
fn listen(args: &mut SyscallRegs) -> Result<usize> {
    let socket_fd = FileDesc(args.arg0 as _);

    return _listen(socket_fd).map(|_| 0);
}

fn _listen(socket_fd: FileDesc) -> Result<()> {
    let socket = fd_query_of::<LocalSocket>(socket_fd)?;
    socket.listener()?;
    Ok(())
}

/// accept a connection on a socket
///
/// [`hyperion_syscall::accept`]
fn accept(args: &mut SyscallRegs) -> Result<usize> {
    let socket_fd = FileDesc(args.arg0 as _);

    return _accept(socket_fd).map(|fd| fd.0);
}

fn _accept(socket_fd: FileDesc) -> Result<FileDesc> {
    let socket = fd_query_of::<LocalSocket>(socket_fd)?;

    // `listen` syscall is not required
    let incoming = socket.listener()?;

    // blocks here
    let pipe = incoming
        .recv()
        .expect("local socket listener send end should never close");

    Ok(fd_push(Arc::new(LocalSocket::connected(socket.info, pipe))))
}

/// connect to a socket
///
/// [`hyperion_syscall::connect`]
fn connect(args: &mut SyscallRegs) -> Result<usize> {
    let socket = FileDesc(args.arg0 as _);
    let addr = read_untrusted_str(args.arg1, args.arg2)?;

    _connect(socket, addr).map(|_| 0)
}

fn _connect(socket_fd: FileDesc, addr: &str) -> Result<()> {
    let client = fd_query_of::<LocalSocket>(socket_fd)?;

    let server = VFS_ROOT
        // TODO: inode
        .find_file(addr, false, false)
        .map_err(map_vfs_err_to_syscall_err)?
        .lock_arc()
        .as_any()
        .downcast_ref::<BoundSocket>()
        .ok_or(Error::CONNECTION_REFUSED)?
        .0
        .clone();

    if !client.is_uninit() {
        return Err(Error::INVALID_ARGUMENT);
    }
    let listener = server.listener()?;

    let (conn_client, conn_server) = SocketPipe::new();
    client.connection(conn_client)?;
    listener
        .send(conn_server)
        .map_err(|_| Error::CONNECTION_REFUSED)?;

    Ok(())
}

/// send data to a socket
///
/// [`hyperion_syscall::send`]
pub fn send(args: &mut SyscallRegs) -> Result<usize> {
    let socket = FileDesc(args.arg0 as _);
    let buf = read_untrusted_bytes(args.arg1, args.arg2)?;
    let flags = args.arg3 as _;

    _send(socket, buf, flags)
}

fn _send(socket_fd: FileDesc, buf: &[u8], _flags: usize) -> Result<usize> {
    let socket = fd_query(socket_fd)?;
    socket.write(buf)
}

/// recv data from a socket
///
/// `read` does the exact same thing
///
/// [`hyperion_syscall::recv`]
pub fn recv(args: &mut SyscallRegs) -> Result<usize> {
    let socket = FileDesc(args.arg0 as _);
    let buf = read_untrusted_bytes_mut(args.arg1, args.arg2)?;
    let flags = args.arg3 as _;

    _recv(socket, buf, flags)
}

fn _recv(socket_fd: FileDesc, buf: &mut [u8], _flags: usize) -> Result<usize> {
    let socket = fd_query(socket_fd)?;
    socket.read(buf)
}

/// pid of the current process
///
/// [`hyperion_syscall::get_pid`]
pub fn get_pid(_args: &mut SyscallRegs) -> Result<usize> {
    Ok(process().pid.num())
}

/// tid of the current thread
///
/// [`hyperion_syscall::get_tid`]
pub fn get_tid(_args: &mut SyscallRegs) -> Result<usize> {
    Ok(task().tid.num())
}

/// duplicate a file descriptor
///
/// [`hyperion_syscall::dup`]
pub fn dup(args: &mut SyscallRegs) -> Result<usize> {
    let old = FileDesc(args.arg0 as _);
    let new = FileDesc(args.arg1 as _);

    let old = fd_query(old)?;
    fd_replace(new, old);

    Ok(new.0 as _)
}

/// futex wait
///
/// [`hyperion_syscall::futex_wait`]
pub fn futex_wait(args: &mut SyscallRegs) -> Result<usize> {
    let addr = read_untrusted_ref::<AtomicUsize>(args.arg0)?;
    let val = args.arg1 as usize;

    futex::wait(addr, val);

    return Ok(0);
}

/// futex wake
///
/// [`hyperion_syscall::futex_wake`]
pub fn futex_wake(args: &mut SyscallRegs) -> Result<usize> {
    let addr = read_untrusted_ref::<AtomicUsize>(args.arg0)?;
    let num = args.arg1 as usize;

    futex::wake(addr, num);

    return Ok(0);
}

/// map file to memory
///
/// [`hyperion_syscall::map_file`]
pub fn map_file(args: &mut SyscallRegs) -> Result<usize> {
    let fd = FileDesc(args.arg0 as _);
    let at = NonNull::new(args.arg1 as *mut ());
    let size = args.arg2 as usize;
    let offset = args.arg3 as usize;

    if let Some(at) = at {
        error!("handle map_file at {at:?}");
    }
    if offset != 0 {
        error!("handle map_file offset {offset:?}");
    }

    let mut file = fd_query_of::<FileDescData>(fd)?.file_ref.lock_arc();

    let phys: Box<[PageFrame]> = file.map_phys(size).map_err(map_vfs_err_to_syscall_err)?;

    let this = process();

    let size = align_up(size as _, 0x1000);
    let bottom = align_down(
        this.heap_bottom.fetch_add(size as usize, Ordering::SeqCst) as u64,
        0x1000,
    );

    let mut offs = 0u64;
    for phys in phys.into_vec() {
        let bottom = VirtAddr::new(bottom) + offs;
        debug!("mapping phys page {:018x}", phys.virtual_addr());
        this.address_space.page_map.map(
            bottom..bottom + size,
            phys.physical_addr(),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
        );
        offs += size;
    }

    Ok(bottom as usize)
}

/// unmap file from memory
///
/// [`hyperion_syscall::unmap_file`]
pub fn unmap_file(args: &mut SyscallRegs) -> Result<usize> {
    let fd = FileDesc(args.arg0 as _);
    // let at = NonNull::new(args.arg1 as *mut ());
    // let size = args.arg2 as usize;

    // hyperion_log::debug!("unmap_file({file:?}, {at:?}, {size})");

    let mut file = fd_query_of::<FileDescData>(fd)?.file_ref.lock_arc();

    file.unmap_phys().map_err(map_vfs_err_to_syscall_err)?;

    Ok(0)
}

/// get file metadata
///
/// [`hyperion_syscall::metadata`]
pub fn metadata(args: &mut SyscallRegs) -> Result<usize> {
    // hyperion_log::debug!("metadata: a0:{} a1:{:#x}", args.arg0, args.arg1);
    let fd = FileDesc(args.arg0 as _);
    let meta: &mut Metadata = read_untrusted_mut(args.arg1)?;

    let file = fd_query(fd)?;
    meta.len = file.len()?;
    meta.position = file.seek(0, Seek::CUR)?;

    Ok(0)
}

/// set file position
///
/// [`hyperion_syscall::seek`]
pub fn seek(args: &mut SyscallRegs) -> Result<usize> {
    let fd = FileDesc(args.arg0 as _);
    let offset = args.arg1 as isize;
    let origin = Seek(args.arg2 as usize);

    let file = fd_query(fd)?;
    file.seek(offset, origin)?;

    Ok(0)
}

/// launch a binary
///
/// [`hyperion_syscall::seek`]
pub fn system(args: &mut SyscallRegs) -> Result<usize> {
    let program: &str = read_untrusted_str(args.arg0, args.arg1)?;
    // FIXME: &str (&[u8]) is not yet ABI stable
    let args: &[&str] = read_untrusted_slice(args.arg2, args.arg3)?;

    let program = program.to_string();
    let args = args
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>();

    // hyperion_log::debug!("system `{program}` `{args:?}`");

    static NULL_STDIO: Lazy<Arc<dyn FileDescriptor>> =
        Lazy::new(|| Arc::new(FileDescData::open("/dev/null").unwrap()));

    let stdin = NULL_STDIO.clone();
    // let stdout = NULL_STDIO.clone();
    // let stderr = NULL_STDIO.clone();
    let stdout = fd_query(FileDesc(1))?;
    let stderr = fd_query(FileDesc(2))?;

    schedule(move || {
        let mut elf = Vec::new();

        let bin = VFS_ROOT
            .find_file(program.as_str(), false, false)
            .expect("could not load ELF");

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

        // set its name
        hyperion_scheduler::rename(program.as_str());

        // setup the STDIO
        hyperion_kernel_impl::fd_replace(FileDesc(0), stdin);
        hyperion_kernel_impl::fd_replace(FileDesc(1), stdout);
        hyperion_kernel_impl::fd_replace(FileDesc(2), stderr);

        // setup the environment
        let args: Vec<&str> = [program.as_str()] // TODO: actually load binaries from vfs
            .into_iter()
            .chain(args.iter().flat_map(|args| args.split(' ')))
            .collect();
        let args = &args[..];

        debug!("spawning \"{program}\" with args {args:?}");

        // load ..
        let loader = Loader::new(elf.as_ref());
        loader.load();

        // .. and exec the binary
        if loader.enter_userland(args).is_err() {
            error!("no ELF entrypoint");
            let stderr = fd_query(FileDesc(2)).unwrap();
            stderr.write(b"invalid ELF: entry point missing").unwrap();
        }
    });

    Ok(0)
}
