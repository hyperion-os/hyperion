use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};
use core::{
    any::Any,
    mem,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use hyperion_arch::{syscall::SyscallRegs, vmm::HIGHER_HALF_DIRECT_MAPPING};
use hyperion_drivers::{log::KernelLogs, null::Null};
use hyperion_futures::{
    lazy::{Lazy, Once},
    lock::Mutex,
    map::{self, AsyncHashMap},
    mpmc::Channel,
};
use hyperion_log::*;
use hyperion_mem::{
    buf::{Buffer, BufferMut},
    vmm::PageMapImpl,
};
use hyperion_scheduler::{
    proc::Process,
    task::{RunnableTask, Task},
};
use hyperion_syscall::{
    err::{Error, Result},
    fs::FileOpenFlags,
    id,
};
use hyperion_vfs::{
    node::{DirDriverExt, FileDriver, FileDriverExt, FileNode, Ref},
    tmpfs::TmpFs,
    OpenOptions,
};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub static TASKS: Channel<SyscallRegs> = Channel::new();

//

pub fn syscall(args: &mut SyscallRegs) {
    match args.syscall_id as usize {
        id::LOG => log(args),
        id::EXIT => exit(args),
        id::DONE => done(args),
        id::YIELD_NOW => yield_now(args),
        // id::TIMESTAMP => {},
        // id::NANOSLEEP => {},
        // id::NANOSLEEP_UNTIL => {},
        // id::SPAWN => {},
        id::PALLOC => palloc(args),
        // id::PFREE => {},
        // id::SEND => {},
        // id::RECV => {},
        // id::RENAME => {},
        //
        id::OPEN => open(args),
        id::CLOSE => close(args),
        id::READ => read(args),
        id::WRITE => write(args),

        // id::SOCKET => {},
        // id::BIND => {},
        // id::LISTEN => {},
        // id::ACCEPT => {},
        // id::CONNECT => {},
        //
        id::GET_PID => get_pid(args),
        id::GET_TID => get_tid(args),

        // id::DUP => {},
        // id::PIPE => {},
        id::FUTEX_WAIT => futex_wait(args),
        id::FUTEX_WAKE => futex_wake(args),

        // id::MAP_FILE => {},
        // id::UNMAP_FILE => {},
        // id::METADATA => {},
        // id::SEEK => {},

        // id::SYSTEM => {},
        // id::FORK => {},
        // id::WAITPID => {},
        other => {
            debug!("invalid syscall ({other})");
            *args = RunnableTask::next().set_active();
            return;
        }
    };
}

fn set_result(args: &mut SyscallRegs, result: Result<usize>) {
    args.syscall_id = Error::encode(result) as u64;
}

/// print a string to logs
///
/// [`hyperion_syscall::log`]
pub fn log(args: &mut SyscallRegs) {
    set_result(
        args,
        try {
            // FIXME: lock the page table, or these specific pages during this print
            // because otherwise a second thread could free these pages => cross-process use after free
            let str = read_untrusted_str(args.arg0, args.arg1)?;
            hyperion_log::print!("{str}");
            0
        },
    );
}

/// [`hyperion_syscall::exit`]
pub fn exit(args: &mut SyscallRegs) {
    // FIXME: kill the whole process and stop other tasks in it using IPI
    *args = RunnableTask::next().set_active();
}

/// [`hyperion_syscall::done`]
pub fn done(args: &mut SyscallRegs) {
    *args = RunnableTask::next().set_active();
}

/// [`hyperion_syscall::yield_now`]
pub fn yield_now(args: &mut SyscallRegs) {
    set_result(args, Ok(0));

    let Some(next) = RunnableTask::try_next() else {
        return;
    };
    let current = RunnableTask::active(args.clone());

    *args = next.set_active();
    current.ready();
}

/// [`hyperion_syscall::palloc`]
pub fn palloc(args: &mut SyscallRegs) {
    let n_pages = args.arg0 as usize;
    let flags = PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;

    let result = Process::current()
        .unwrap()
        .alloc(n_pages, flags)
        .map(|ptr| ptr.as_u64() as usize)
        .map_err(|_| Error::OUT_OF_VIRTUAL_MEMORY);

    set_result(args, result);
}

async fn vfs_init() {
    static VFS_INIT: Once<()> = Once::new();
    VFS_INIT
        .call_once(async move {
            // hyperion_vfs::mount(None, "/", TmpFs::new().into_dir_ref())
            //     .await
            //     .unwrap();
            hyperion_vfs::bind(None, "/dev/null", Null.into_file_ref())
                .await
                .unwrap();
            hyperion_vfs::bind(None, "/dev/log", KernelLogs.into_file_ref())
                .await
                .unwrap();
        })
        .await;
}

/// [`hyperion_syscall::open`]
pub fn open(args: &mut SyscallRegs) {
    let ptr = args.arg0;
    let len = args.arg1;
    let flags = args.arg2;
    let _mode = args.arg3;

    let flags = FileOpenFlags::from_bits_truncate(flags as usize);
    let opts = OpenOptions::from_flags(flags);

    let err: Result<()> = try {
        if len >= 0x1000 {
            Err(Error::INVALID_ARGUMENT)?;
        }

        let path: Box<str> = String::from_utf8(read_untrusted_bytes(ptr, len)?.into())
            .map_err(|_| Error::INVALID_UTF8)?
            .into();

        let mut task = RunnableTask::active(args.clone());

        hyperion_futures::spawn(async move {
            vfs_init().await;

            let result = try {
                if flags.contains(FileOpenFlags::IS_DIR) {
                    Err(Error::UNIMPLEMENTED)?;
                }

                let file = hyperion_vfs::get(Some(&task.task.process), path.as_ref(), opts)
                    .await
                    .and_then(|node| node.to_file().ok_or(Error::NOT_A_FILE))?
                    .driver
                    .lock()
                    .await
                    .clone();

                fd_push(&task.task.process, file).await as usize
            };

            set_result(&mut task.trap, result);
            task.ready();
        });
    };

    if let Err(err) = err {
        set_result(args, Err(err));
        return;
    }

    *args = RunnableTask::next().set_active();
}

/// [`hyperion_syscall::close`]
pub fn close(args: &mut SyscallRegs) {
    let fd = args.arg0;

    let mut task = RunnableTask::active(args.clone());

    hyperion_futures::spawn(async move {
        let res = fd_remove(&task.task.process, fd)
            .await
            .ok_or(Error::BAD_FILE_DESCRIPTOR)
            .map(|_| 0);

        set_result(&mut task.trap, res);
        task.ready();
    });

    *args = RunnableTask::next().set_active();
}

/// [`hyperion_syscall::read`]
pub fn read(args: &mut SyscallRegs) {
    let fd = args.arg0;
    let ptr = args.arg1;
    let len = args.arg2;

    let mut prev = RunnableTask::active(args.clone());

    hyperion_futures::spawn(async move {
        let fd = fd_get(&prev.task.process, fd).await;

        let result = try {
            let mut fd = fd.ok_or(Error::BAD_FILE_DESCRIPTOR)?;

            let mut buffer =
                unsafe { BufferMut::new(&prev.task.process.address_space, ptr as _, len as _) };

            fd.file.read(Some(&prev.task.process), 0, buffer).await?
        };

        set_result(&mut prev.trap, result);
        prev.ready();
    });

    *args = RunnableTask::next().set_active();
}

/// [`hyperion_syscall::write`]
pub fn write(args: &mut SyscallRegs) {
    let fd = args.arg0;
    let ptr = args.arg1;
    let len = args.arg2;

    let mut prev = RunnableTask::active(args.clone());

    hyperion_futures::spawn(async move {
        let fd = fd_get(&prev.task.process, fd).await;

        let result = try {
            let mut fd = fd.ok_or(Error::BAD_FILE_DESCRIPTOR)?;

            let buffer =
                unsafe { Buffer::new(&prev.task.process.address_space, ptr as _, len as _) };

            fd.file.write(Some(&prev.task.process), 0, buffer).await?
        };

        set_result(&mut prev.trap, result);
        prev.ready();
    });

    *args = RunnableTask::next().set_active();
}

/// [`hyperion_syscall::get_pid`]
pub fn get_pid(args: &mut SyscallRegs) {
    set_result(args, Ok(Process::current().unwrap().pid.num()));
}

/// [`hyperion_syscall::get_tid`]
pub fn get_tid(args: &mut SyscallRegs) {
    set_result(args, Ok(Task::current().unwrap().tid.num()));
}

static FUTEX_MAP: AsyncHashMap<u64, FutexEntry>;

struct FutexEntry {
    this: RunnableTask,
    next: Option<Box<FutexEntry>>,
}

/// [`hyperion_syscall::futex_wait`]
pub fn futex_wait(args: &mut SyscallRegs) {
    let addr = args.arg0;
    let val = args.arg1;

    let result: Result<()> = try {
        let futex: &AtomicUsize = read_untrusted_ref(addr)?;

        futex;
    };

    Process::current()
        .unwrap()
        .address_space
        .virt_to_phys(VirtAddr::from_ptr());

    FUTEX_MAP.get();

    set_result(args, Ok(0));
}

/// [`hyperion_syscall::futex_wake`]
pub fn futex_wake(args: &mut SyscallRegs) {
    set_result(args, Ok(0));
}

//

#[derive(Default)]
struct ProcessExt {
    fds: AsyncHashMap<u64, FileDescriptor>,
    next_fd: AtomicU64,
}

impl hyperion_scheduler::proc::ProcessExt for ProcessExt {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn close(&self) {}
}

//

struct FileDescriptor {
    // readonly: bool,
    file: Ref<dyn FileDriver>,
}

//

pub fn process_ext(proc: &Process) -> &ProcessExt {
    proc.ext
        .call_once(|| Box::new(ProcessExt::default()))
        .as_any()
        .downcast_ref()
        .unwrap()
}

pub async fn fd_insert(proc: &Process, fd: u64, file: Ref<dyn FileDriver>) {
    let proc_ext = process_ext(proc);
    proc_ext.fds.insert(fd, FileDescriptor { file }).await;
}

pub async fn fd_push(proc: &Process, file: Ref<dyn FileDriver>) -> u64 {
    let proc_ext = process_ext(proc);
    let fd = proc_ext.next_fd.fetch_add(1, Ordering::Relaxed);
    proc_ext.fds.insert(fd, FileDescriptor { file }).await;
    fd
}

pub async fn fd_get(proc: &Process, fd: u64) -> Option<map::Ref<u64, FileDescriptor>> {
    let proc_ext = process_ext(proc);
    proc_ext.fds.get(&fd).await
}

pub async fn fd_remove(proc: &Process, fd: u64) -> Option<map::Ref<u64, FileDescriptor>> {
    let proc_ext = process_ext(proc);
    proc_ext.fds.remove(&fd).await
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

    if end >= HIGHER_HALF_DIRECT_MAPPING {
        // lower half pages are always safe to read and write,
        // kernel code giving a page fault in lower half terminates that process with a SIGSEGV
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
