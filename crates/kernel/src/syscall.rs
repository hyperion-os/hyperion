use alloc::{
    boxed::Box,
    collections::btree_map::{BTreeMap, Entry},
};
use core::{
    any::Any,
    mem::{self, MaybeUninit},
    str,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use hyperion_arch::syscall::SyscallRegs;
use hyperion_futures::{map::LazyHasher, mpmc::Channel, rwlock::RwLock};
use hyperion_mem::{
    buf::{Buffer, BufferMut},
    vmm::{MapFlags, MapTarget, PageMapImpl},
};
use hyperion_scheduler::{
    proc::Process,
    task::{RunnableTask, Task},
};
use hyperion_syscall::{
    err::{Error, Result},
    fs::FileOpenFlags,
    Id, MemMapFlags,
};
use hyperion_vfs::{
    node::{FileDriver, Node, Ref},
    OpenOptions,
};
use x86_64::{align_down, structures::paging::PageTableFlags, PhysAddr, VirtAddr};

//

pub static TASKS: Channel<SyscallRegs> = Channel::new();

//

pub fn syscall(args: &mut SyscallRegs) {
    let Ok(syscall) = Id::try_from(args.syscall_id as usize) else {
        hyperion_log::debug!("invalid syscall from user-space: {}", args.syscall_id);
        set_result(args, Err(Error::INVALID_ARGUMENT));
        return;
    };

    hyperion_log::trace!(
        "syscall={syscall:?} {:?} CPU-{}",
        [args.arg0, args.arg1, args.arg2, args.arg3, args.arg4],
        hyperion_arch::cpu_id(),
    );

    match syscall {
        Id::Log => log(args),
        Id::Exit => exit(args),
        Id::Done => done(args),
        Id::YieldNow => yield_now(args),
        // Id::TIMESTAMP => {},
        // Id::NANOSLEEP => {},
        // Id::NANOSLEEP_UNTIL => {},
        Id::Spawn => spawn(args),
        // Id::SEND => {},
        // Id::RECV => {},
        // Id::RENAME => {},
        //
        Id::Open => open(args),
        Id::Close => close(args),
        Id::Read => read(args),
        Id::Write => write(args),

        // Id::SOCKET => {},
        // Id::BIND => {},
        // Id::LISTEN => {},
        // Id::ACCEPT => {},
        // Id::CONNECT => {},
        //
        Id::GetPid => get_pid(args),
        Id::GetTid => get_tid(args),

        // Id::DUP => {},
        // Id::PIPE => {},
        Id::FutexWait => futex_wait(args),
        Id::FutexWake => futex_wake(args),

        Id::MemMap => mem_map(args),
        Id::MemUnmap => mem_unmap(args),
        // Id::METADATA => {},
        // Id::SEEK => {},

        // Id::SYSTEM => {},
        Id::Fork => fork(args),
        // Id::WAITPID => {},
        other => {
            hyperion_log::error!("unimplemented syscall ({other:?})");
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

/// [`hyperion_syscall::spawn`]
pub fn spawn(args: &mut SyscallRegs) {
    let ip = args.arg0;
    let sp = args.arg1;

    hyperion_log::trace!("spawn({ip:#x}, {sp:#x})");
    RunnableTask::new_in(ip, sp, Process::current().unwrap()).ready();

    set_result(args, Ok(0));
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

        let path = str::from_utf8(read_untrusted_bytes(ptr, len)?.into())
            .map_err(|_| Error::INVALID_UTF8)?;

        // copy the path to kernel memory
        let path: Box<str> = path.into();

        let mut task = RunnableTask::active(args.clone());

        hyperion_futures::spawn(async move {
            let result = try {
                let node = hyperion_vfs::get(Some(&task.task.process), path.as_ref(), opts).await?;
                let is_dir = flags.contains(FileOpenFlags::IS_DIR);

                let file = match (node, is_dir) {
                    (Node::File(file), false) => file.driver.lock().await.clone(),
                    (Node::Dir(dir), true) => dir.as_opendir(),
                    (_, false) => Err(Error::NOT_A_FILE)?,
                    (_, true) => Err(Error::NOT_A_DIRECTORY)?,
                };

                fd_push(&task.task.process, file).await as usize
            };

            hyperion_log::trace!("open(\"{path}\", {flags:?}, {_mode:?}) => {result:?}");
            set_result(&mut task.trap, result);
            task.ready();
        });
    };

    if let Err(err) = err {
        hyperion_log::trace!("open(??, {flags:?}, {_mode:?}) => Err({err:?})");
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
            let fd = fd.ok_or(Error::BAD_FILE_DESCRIPTOR)?;

            let buffer =
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
            let fd = fd.ok_or(Error::BAD_FILE_DESCRIPTOR)?;

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

// 32 pages for the futex map
static FUTEX_MAP: [spin::Mutex<BTreeMap<PhysAddr, Box<FutexEntry>>>; 0x1000] =
    [const { spin::Mutex::new(BTreeMap::new()) }; 0x1000];
static FUTEX_MAP_HASHER: LazyHasher = LazyHasher::new();

struct FutexEntry {
    this: RunnableTask,
    next: Option<Box<FutexEntry>>,
}

/// [`hyperion_syscall::futex_wait`]
pub fn futex_wait(args: &mut SyscallRegs) {
    let addr = args.arg0;
    let val = args.arg1;

    let futex: &AtomicUsize = match read_untrusted_ref(addr) {
        Ok(v) => v,
        Err(err) => {
            set_result(args, Err(err));
            return;
        }
    };

    let (addr, flags) = Process::current()
        .unwrap()
        .address_space
        .virt_to_phys(VirtAddr::from_ptr(futex))
        .unwrap(); // FIXME: segfault the process, instead of crashing the kernel

    if !flags.contains(PageTableFlags::USER_ACCESSIBLE) {
        todo!("segfault"); // FIXME: same as above
    }

    let entry: Box<MaybeUninit<FutexEntry>> = Box::new_uninit(); // preallocate cuz it could be slow, spinlock doesnt like slow things
    let hash = hyperion_futures::block_on(FUTEX_MAP_HASHER.hash(&addr));
    let mut tree = FUTEX_MAP[hash as usize % FUTEX_MAP.len()].lock();

    // check if sleeping or not
    if futex.load(Ordering::SeqCst) != val as usize {
        set_result(args, Ok(0));
        return;
    }

    // sleeping for sure
    let mut entry = Box::write(
        entry,
        FutexEntry {
            this: RunnableTask::active(args.clone()),
            next: None,
        },
    );

    match tree.entry(addr) {
        Entry::Occupied(mut occupied_entry) => {
            let val: &mut Box<FutexEntry> = occupied_entry.get_mut();
            mem::swap(val, &mut entry);
            val.next = Some(entry);
        }
        Entry::Vacant(vacant_entry) => {
            vacant_entry.insert(entry);
        }
    }

    *args = RunnableTask::next().set_active();
    return;
}

/// [`hyperion_syscall::futex_wake`]
pub fn futex_wake(args: &mut SyscallRegs) {
    let addr = args.arg0;
    let num = args.arg1;

    if num == 0 {
        return;
    }

    let futex: &AtomicUsize = match read_untrusted_ref(addr) {
        Ok(v) => v,
        Err(err) => {
            set_result(args, Err(err));
            return;
        }
    };

    let (addr, flags) = Process::current()
        .unwrap()
        .address_space
        .virt_to_phys(VirtAddr::from_ptr(futex))
        .unwrap(); // FIXME: segfault the process, instead of crashing the kernel

    if !flags.contains(PageTableFlags::USER_ACCESSIBLE) {
        todo!("segfault"); // FIXME: same as above
    }

    let hash = hyperion_futures::block_on(FUTEX_MAP_HASHER.hash(&addr));
    let mut tree = FUTEX_MAP[hash as usize % FUTEX_MAP.len()].lock();

    set_result(args, Ok(0));

    match tree.entry(addr) {
        Entry::Vacant(..) => {}
        Entry::Occupied(mut occupied_entry) => {
            let first = occupied_entry.get_mut();

            for _ in 0..num {
                if let Some(next) = first.next.take() {
                    mem::replace(first, next).this.ready();
                } else {
                    occupied_entry.remove().this.ready();
                    break;
                }
            }
        }
    }
}

/// [`hyperion_syscall::mem_map`]
fn mem_map(args: &mut SyscallRegs) {
    let result = _mem_map(args.arg0, args.arg1, args.arg2, args.arg3, args.arg4);
    hyperion_log::trace!("mem_map => {result:?}");
    set_result(args, result);
}

fn _mem_map(addr: u64, size: u64, flags: u64, fd: u64, offset: u64) -> Result<usize> {
    let flags = MemMapFlags::from_bits_truncate(flags as u32);

    hyperion_log::trace!("mem_map({addr}, {size}, {flags:?}, {fd}, {offset})");

    let start = align_down(addr, 0x1000);
    let end = align_down(
        addr.checked_add(size).ok_or(Error::INVALID_ADDRESS)?,
        0x1000,
    );

    let (addr, len) = read_slice_parts(start, end - start)?;

    if !flags.contains(MemMapFlags::ANON) {
        todo!("mem_map called without ANON");
    }

    let mut mem_flags = MapFlags::USER;
    if flags.contains(MemMapFlags::WRITE) {
        mem_flags |= MapFlags::WRITE;
    }
    if flags.contains(MemMapFlags::EXEC) {
        mem_flags |= MapFlags::EXEC;
    }
    if flags.contains(MemMapFlags::READ) {
        mem_flags |= MapFlags::READ;
    }

    _mem_map_anon(addr, len, mem_flags);

    Ok(start as _)

    // proc.address_space.map(VirtAddr::new(addr), p_addr, flags);

    // hyperion_futures::spawn(async move {
    //     let mut maps = proc.maps.lock().await;

    //     maps.range(range)
    // });

    // TODO: error handling
    // proc.alloc_at(n_pages, , flags);

    // Ok(0)
}

fn _mem_map_anon(addr: VirtAddr, len: usize, flags: MapFlags) {
    let proc = Process::current().unwrap();

    proc.address_space
        .map(addr, len, MapTarget::LazyAlloc, flags);
}

/// [`hyperion_syscall::mem_unmap`]
fn mem_unmap(args: &mut SyscallRegs) {
    let addr = args.arg0;
    let size = args.arg1;

    hyperion_log::trace!("mem_unmap({addr}, {size})");
}

/// [`hyperion_syscall::fork`]
fn fork(args: &mut SyscallRegs) {
    let mut task = RunnableTask::active(args.clone());

    hyperion_futures::spawn(async move {
        let ext = process_ext(&task.task.process).fork().await;

        let mut other = task.fork(Box::new(ext)).await;

        set_result(&mut other.trap, Ok(0));
        set_result(&mut task.trap, Ok(other.task.process.pid.num()));

        // hyperion_log::debug!(
        //     "fork ready0={} ready1={}",
        //     other.task.process.pid,
        //     task.task.process.pid,
        // );

        other.ready();
        task.ready();
    });

    *args = RunnableTask::next().set_active();
}

//

#[derive(Default)]
struct ProcessExt {
    // fds: AsyncHashMap<u64, FileDescriptor>,
    fds: RwLock<BTreeMap<u64, FileDescriptor>>,
    next_fd: AtomicU64,
}

impl ProcessExt {
    async fn fork(&self) -> Self {
        Self {
            fds: RwLock::new(self.fds.read().await.clone()),
            next_fd: AtomicU64::new(0),
        }
    }
}

impl hyperion_scheduler::proc::ProcessExt for ProcessExt {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn close(&self) {}
}

//

#[derive(Clone)]
struct FileDescriptor {
    // readonly: bool,
    file: Ref<dyn FileDriver>,
}

//

fn process_ext(proc: &Process) -> &ProcessExt {
    proc.ext
        .call_once(|| Box::new(ProcessExt::default()))
        .as_any()
        .downcast_ref()
        .unwrap()
}

pub async fn fd_insert(proc: &Process, fd: u64, file: Ref<dyn FileDriver>) {
    let proc_ext = process_ext(proc);
    proc_ext
        .fds
        .write()
        .await
        .insert(fd, FileDescriptor { file });
}

pub async fn fd_push(proc: &Process, file: Ref<dyn FileDriver>) -> u64 {
    let proc_ext = process_ext(proc);
    let mut fds = proc_ext.fds.write().await;

    // FIXME: denial-of-service
    loop {
        let fd = proc_ext.next_fd.fetch_add(1, Ordering::Relaxed);

        if let Entry::Vacant(vacant_entry) = fds.entry(fd) {
            vacant_entry.insert(FileDescriptor { file });
            return fd;
        }
    }
}

async fn fd_get(proc: &Process, fd: u64) -> Option<FileDescriptor> {
    let proc_ext = process_ext(proc);
    Some(proc_ext.fds.read().await.get(&fd)?.clone())
}

async fn fd_remove(proc: &Process, fd: u64) -> Option<FileDescriptor> {
    let proc_ext = process_ext(proc);
    Some(proc_ext.fds.write().await.remove(&fd)?.clone())
}

// +------------------------+
// | untrusted memory rules |
// +------------------------+
//
// all memory in the lower half is safe to use for the kernel,
// because all lower half memory is user accessible OR not mapped
//
// if it is user accessible, its fine
//
// if it is not mapped, then the kernel page faults
// in the lower half and gives a segfault for the process

pub fn read_slice_parts(ptr: u64, len: u64) -> Result<(VirtAddr, usize)> {
    if len == 0 {
        return Ok((VirtAddr::new_truncate(0), 0));
    }

    let Some(end) = ptr.checked_add(len - 1) else {
        return Err(Error::INVALID_ADDRESS);
    };

    let (Ok(start), Ok(end)) = (VirtAddr::try_new(ptr), VirtAddr::try_new(end)) else {
        return Err(Error::INVALID_ADDRESS);
    };

    if end.as_u64() >= 0x8000_0000_0000 {
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
    if !(ptr as *const T).is_aligned() {
        hyperion_log::debug!("not aligned");
        return Err(Error::INVALID_ADDRESS);
    }

    let len = len
        .checked_mul(mem::size_of::<T>() as _)
        .ok_or(Error::INVALID_ADDRESS)?;
    read_slice_parts(ptr, len).map(|(start, len)| {
        if len == 0 {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(start.as_ptr(), len as _) }
        }
    })
}

pub fn read_untrusted_bytes<'a>(ptr: u64, len: u64) -> Result<&'a [u8]> {
    read_slice_parts(ptr, len).map(|(start, len)| {
        if len == 0 {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(start.as_ptr(), len as _) }
        }
    })
}

pub fn read_untrusted_bytes_mut<'a>(ptr: u64, len: u64) -> Result<&'a mut [u8]> {
    read_slice_parts(ptr, len).map(|(start, len)| {
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
