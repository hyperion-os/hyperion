use core::mem;

use hyperion_arch::{syscall::SyscallRegs, vmm::HIGHER_HALF_DIRECT_MAPPING};
use hyperion_futures::mpmc::Channel;
use hyperion_log::*;
use hyperion_scheduler::{
    proc::Process,
    task::{RunnableTask, Task},
};
use hyperion_syscall::{
    err::{Error, Result},
    id,
};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub static TASKS: Channel<SyscallRegs> = Channel::new();

//

pub fn syscall(args: &mut SyscallRegs) {
    // process syscall args

    // // dispatch / run the syscall

    // let task = RunnableTask::active(*args);

    // hyperion_futures::spawn(async move {
    //     hyperion_futures::timer::sleep(Duration::milliseconds(100)).await;
    //     task.ready();
    // });

    // // block on syscall futures

    // *args = RunnableTask::next().set_active();
    // return;

    // // return to the same or another task

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

        // id::OPEN => {},
        // id::CLOSE => {},
        // id::READ => {},
        // id::WRITE => {},

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

/// [`hyperion_syscall::get_pid`]
pub fn get_pid(args: &mut SyscallRegs) {
    set_result(args, Ok(Process::current().unwrap().pid.num()));
}

/// [`hyperion_syscall::get_tid`]
pub fn get_tid(args: &mut SyscallRegs) {
    set_result(args, Ok(Task::current().unwrap().tid.num()));
}

//

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
