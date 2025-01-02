use alloc::{boxed::Box, string::String, vec::Vec};
use core::{any::Any, mem};

use hyperion_arch::{syscall::SyscallRegs, vmm::HIGHER_HALF_DIRECT_MAPPING};
use hyperion_futures::{lock::Mutex, mpmc::Channel};
use hyperion_log::*;
use hyperion_scheduler::{
    proc::Process,
    task::{RunnableTask, Task},
};
use hyperion_syscall::{
    err::{Error, Result},
    id,
};
use hyperion_vfs::{tmpfs::TmpFs, OpenOptions};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub static TASKS: Channel<SyscallRegs> = Channel::new();

// pub static VFS_INIT: hyperion_futures::mpmc

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
        // id::FUTEX_WAIT => {},
        // id::FUTEX_WAKE => {},

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

/// [`hyperion_syscall::open`]
pub fn open(args: &mut SyscallRegs) {
    let ptr = args.arg0;
    let len = args.arg1;
    let _flags = args.arg2;
    let _mode = args.arg3;

    // hyperion_vfs::mount("/", TmpFs::new()).await.unwrap();

    let err: Result<()> = try {
        if len >= 0x1000 {
            Err(Error::INVALID_ARGUMENT)?;
        }

        let path: Box<str> = String::from_utf8(read_untrusted_bytes(ptr, len)?.into())
            .map_err(|_| Error::INVALID_UTF8)?
            .into();

        let mut task = RunnableTask::active(args.clone());

        hyperion_futures::spawn(async move {
            let file = hyperion_vfs::get(path.as_ref(), OpenOptions::new()).await;

            task.ready();
        });
    };

    if let Err(err) = err {
        set_result(args, Err(Error::UNIMPLEMENTED));
        return;
    }

    *args = RunnableTask::next().set_active();
}

/// [`hyperion_syscall::close`]
pub fn close(args: &mut SyscallRegs) {
    set_result(args, Err(Error::UNIMPLEMENTED));
}

/// [`hyperion_syscall::read`]
pub fn read(args: &mut SyscallRegs) {
    set_result(args, Err(Error::UNIMPLEMENTED));
}

/// [`hyperion_syscall::write`]
pub fn write(args: &mut SyscallRegs) {
    let fd = args.arg0;
    let ptr = args.arg1;
    let len = args.arg2;

    // let mut prev = RunnableTask::active(args.clone());

    // let proc = Process::current().unwrap();
    // hyperion_futures::spawn(async move {
    //     let ext = process_ext(&proc);
    //     let fds = ext.fds.lock().await;

    //     try {
    //         let fd = fds.get(fd as usize).ok_or(Error::BAD_FILE_DESCRIPTOR)?;
    //         let locked_fd = fd.lock().await;
    //     }

    //     prev.ready();
    // });

    set_result(
        args,
        try {
            let bytes = read_untrusted_bytes(ptr, len)?;
            Err(Error::UNIMPLEMENTED)?;
            0
        },
    );
}

/// [`hyperion_syscall::get_pid`]
pub fn get_pid(args: &mut SyscallRegs) {
    set_result(args, Ok(Process::current().unwrap().pid.num()));
}

/// [`hyperion_syscall::get_tid`]
pub fn get_tid(args: &mut SyscallRegs) {
    set_result(args, Ok(Task::current().unwrap().tid.num()));
}

//

#[derive(Clone)]
pub struct SparseVec<T> {
    inner: Vec<Option<T>>,
    // TODO:
    // first_free: usize,
    // free_count: usize,
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

impl<T> Default for SparseVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

//

#[derive(Default)]
struct ProcessExt {
    fds: Mutex<SparseVec<Mutex<()>>>,
}

impl hyperion_scheduler::proc::ProcessExt for ProcessExt {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn close(&self) {}
}

//

pub fn process_ext(proc: &Process) -> &ProcessExt {
    proc.ext
        .call_once(|| Box::new(ProcessExt::default()))
        .as_any()
        .downcast_ref()
        .unwrap()
}

// pub async fn fd_insert(fd: usize) {
//     let fds = process_ext().fds.lock().await;

//     fds.replace(fd, FileDescriptor {});
// }

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
