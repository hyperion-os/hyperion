use alloc::{boxed::Box, string::ToString, vec::Vec};
use core::{
    any::{type_name_of_val, Any},
    sync::atomic::Ordering,
};

use hyperion_arch::{stack::USER_HEAP_TOP, syscall::SyscallRegs, vmm::PageMap};
use hyperion_drivers::acpi::hpet::HPET;
use hyperion_instant::Instant;
use hyperion_log::*;
use hyperion_mem::{
    pmm::{self, PageFrame},
    vmm::PageMapImpl,
};
use hyperion_scheduler::{
    lock::Mutex,
    process,
    task::{Process, ProcessExt},
};
use hyperion_syscall::err::{Error, Result};
use hyperion_vfs::{error::IoError, tree::FileRef};
use time::Duration;
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub fn syscall(args: &mut SyscallRegs) {
    let id = args.syscall_id;
    let (result, name) = match id {
        1 => call_id(log, args),
        2 | 420 => call_id(exit, args),
        3 => call_id(yield_now, args),
        4 => call_id(timestamp, args),
        5 => call_id(nanosleep, args),
        6 => call_id(nanosleep_until, args),
        8 => call_id(pthread_spawn, args),
        9 => call_id(palloc, args),
        10 => call_id(pfree, args),
        11 => call_id(send, args),
        12 => call_id(recv, args),
        13 => call_id(rename, args),

        1000 => call_id(open, args),
        1100 => call_id(close, args),
        1200 => call_id(read, args),

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
    //     "syscall-{name}-{}({}, {}, {}, {}, {})",
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

    if hyperion_scheduler::send(pid, data.to_vec().into()).is_err() {
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
    return Ok(hyperion_scheduler::recv_to(buf));
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

    let this = process();
    let ext = process_ext_with(&this);

    let file_ref = hyperion_vfs::open(path, false, false).map_err(map_vfs_err_to_syscall_err)?;
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
///  - `0` :
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

struct ProcessExtra {
    files: Mutex<Vec<Option<File>>>,
}

struct File {
    file_ref: FileRef,
    position: usize,
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
