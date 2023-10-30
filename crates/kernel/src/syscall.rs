use core::{any::type_name_of_val, sync::atomic::Ordering};

use hyperion_arch::{stack::USER_HEAP_TOP, syscall::SyscallRegs, vmm::PageMap};
use hyperion_drivers::acpi::hpet::HPET;
use hyperion_instant::Instant;
use hyperion_log::*;
use hyperion_mem::{
    pmm::{self, PageFrame},
    vmm::PageMapImpl,
};
use time::Duration;
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub fn syscall(args: &mut SyscallRegs) {
    let id = args.syscall_id;
    let (result, name): (i64, &str) = match id {
        1 => call_id(log, args),
        2 | 420 => call_id(exit, args),
        3 => call_id(yield_now, args),
        4 => call_id(timestamp, args),
        5 => call_id(nanosleep, args),
        6 => call_id(nanosleep_until, args),
        7 => call_id(open, args),
        8 => call_id(pthread_spawn, args),
        9 => call_id(palloc, args),
        10 => call_id(pfree, args),
        11 => call_id(send, args),
        12 => call_id(recv, args),

        _ => {
            debug!("invalid syscall");
            // invalid syscall id, kill the process as a f u
            args.syscall_id = 2;
            args.arg0 = i64::MIN as _;
            exit(args);
            (2, "invalid")
        }
    };

    _ = (result, name);
    /* if result < 0 {
        debug!("syscall `{name}` (id {id}) returned {result}",);
    } */
}

fn call_id(f: impl FnOnce(&mut SyscallRegs) -> i64, args: &mut SyscallRegs) -> (i64, &str) {
    let name = type_name_of_val(&f);
    let res = f(args);
    args.syscall_id = res as _;
    (res, name)
}

/// print a string to logs
///
/// # arguments
///  - `syscall_id` : 1
///  - `arg0` : _utf8 string address_
///  - `arg1` : _utf8 string length_
///
/// # return codes (in syscall_id after returning)
///  - `0` : ok
///  - `1` : invalid address range (arg0 .. arg1)
///  - `2` : address range not mapped for the user (arg0 .. arg1)
///  - `3` : invalid utf8
pub fn log(args: &mut SyscallRegs) -> i64 {
    let str = match read_untrusted_str(args.arg0, args.arg1) {
        Ok(v) => v,
        Err(err) => return err,
    };

    hyperion_log::print!("{str}");
    return 0;
}

/// exit and kill the current process
///
/// # arguments
///  - `syscall_id` : 2
///  - `arg0` : _exit code_
///
/// # return codes (in syscall_id after returning)
/// _won't return_
pub fn exit(_args: &mut SyscallRegs) -> i64 {
    // TODO: exit code
    hyperion_scheduler::stop();
}

/// give the processor back to the kernel temporarily
///
/// # arguments
///  - `syscall_id` : 3
///
/// # return codes (in syscall_id after returning)
///  - `0` : ok
pub fn yield_now(_args: &mut SyscallRegs) -> i64 {
    hyperion_scheduler::yield_now();
    return 0;
}

/// get the number of nanoseconds after boot
///
/// # arguments
///  - `syscall_id` : 4
///
/// # return values
///  - `arg0` : lower 64 bits of the 128 bit timestamp
///  - `arg1` : upper 64 bits of the 128 bit timestamp
///
/// # return codes (in syscall_id after returning)
///  - `0` : ok
pub fn timestamp(args: &mut SyscallRegs) -> i64 {
    let nanos = HPET.nanos();

    /* let bytes = nanos.to_ne_bytes();
    args.arg0 = u64::from_ne_bytes(bytes[0..8].try_into().unwrap());
    args.arg1 = u64::from_ne_bytes(bytes[8..16].try_into().unwrap()); */
    args.arg0 = nanos as u64;
    args.arg1 = (nanos >> 64) as u64;

    return 0;
}

/// sleep at least arg0|arg1 nanoseconds
///
/// # arguments
///  - `syscall_id` : 5
///  - `arg0` : lower 64 bits of the 128 bit duration
///  - `arg1` : _todo_
///
/// # return codes (in syscall_id after returning)
///  - `0` : ok
pub fn nanosleep(args: &mut SyscallRegs) -> i64 {
    hyperion_scheduler::sleep(Duration::nanoseconds((args.arg0 as i64).max(0)));
    return 0;
}

/// sleep at least until the nanosecond arg0|arg1 happens
///
/// # arguments
///  - `syscall_id` : 6
///  - `arg0` : lower 64 bits of the 128 bit timestamp
///  - `arg1` : _todo_
///
/// # return codes (in syscall_id after returning)
///  - `0` : ok
pub fn nanosleep_until(args: &mut SyscallRegs) -> i64 {
    hyperion_scheduler::sleep_until(Instant::new(args.arg0 as u128));
    return 0;
}

/// open a file
///
/// # arguments
///  - `syscall_id` : 7
///  - `arg0` : filename : _utf8 string address_
///  - `arg1` : filename : _utf8 string length_
///
/// # return codes (in syscall_id after returning)
///  - `-3` : invalid utf8
///  - `-2` : address range not mapped for the user (arg0 .. arg1)
///  - `-1` : invalid address range (arg0 .. arg1)
///  - `0..` :
pub fn open(_args: &mut SyscallRegs) -> i64 {
    /* let path = match read_untrusted_str(args.arg0, args.arg1) {
        Ok(v) => v,
        Err(err) => return err,
    }; */

    // hyperion_vfs::open(path, false, false);

    return -1 as _;
}

/// spawn a new thread
///
/// thread entry signature: `extern "C" fn thread_entry(stack_ptr: u64, arg1: u64) -> !`
///
/// # arguments
///  - `syscall_id` : 8
///  - `arg0` : the thread function pointer
///  - `arg1` : the thread function argument
pub fn pthread_spawn(args: &mut SyscallRegs) -> i64 {
    hyperion_scheduler::spawn_userspace(args.arg0, args.arg1);
    return 0;
}

/// allocate physical pages and map them to virtual memory
///
/// # arguments
///  - `syscall_id` : 9
///  - `arg0` : page count
///
/// # return codes (in syscall_id after returning)
///  - `-2` : out of virtual memory
///  - `-1` : out of memory
///  - `0..` : virtual alloc address
pub fn palloc(args: &mut SyscallRegs) -> i64 {
    let pages = args.arg0 as usize;
    let alloc = pages * 0x1000;

    let active = hyperion_scheduler::process();
    let mut allocs = active.allocs.bitmap();
    let alloc_bottom = active.heap_bottom.fetch_add(alloc, Ordering::SeqCst);
    let alloc_top = alloc_bottom + alloc;

    if alloc_top as u64 >= USER_HEAP_TOP {
        return -2;
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

    return alloc_bottom as _;
}

/// free allocated physical pages
///
/// # arguments
///  - `syscall_id` : 10
///  - `arg0` : page
///  - `arg1` : page count
///
/// # return codes (in syscall_id after returning)
///  - `-2` : invalid ptr
///  - `-1` : invalid alloc
///  - `0`  : ok
pub fn pfree(args: &mut SyscallRegs) -> i64 {
    let Ok(alloc_bottom) = VirtAddr::try_new(args.arg0) else {
        return -2;
    };
    let pages = args.arg1 as usize;

    let active = hyperion_scheduler::process();
    let mut allocs = active.allocs.bitmap();

    let page_bottom = alloc_bottom.as_u64() as usize / 0x1000;
    for page in page_bottom..page_bottom + pages {
        if !allocs.get(page).unwrap() {
            return -1;
        }

        allocs.set(page, false).unwrap();
    }

    let Some(palloc) = active.address_space.page_map.virt_to_phys(alloc_bottom) else {
        return -2;
    };

    let frames = unsafe { PageFrame::new(palloc, pages) };
    pmm::PFA.free(frames);
    active
        .address_space
        .page_map
        .unmap(alloc_bottom..alloc_bottom + pages * 0x1000);

    return 0;
}

/// send data to an input channel of a process
///
/// # arguments
///  - `syscall_id` : 11
///  - `arg0`       : target PID
///  - `arg1`       : data ptr
///  - `arg2`       : data len (bytes)
///
/// # return codes (in syscall_id after returning)
///  - `-3` : no such process
///  - `-2` : address range not mapped for the user (arg0 .. arg1)
///  - `-1` : invalid address range (arg0 .. arg1)
///  -  `0` : ok
pub fn send(args: &mut SyscallRegs) -> i64 {
    let target_pid = args.arg0;
    let data = match read_untrusted_bytes(args.arg1, args.arg2) {
        Ok(v) => v,
        Err(err) => {
            return err;
        }
    };

    let pid = hyperion_scheduler::task::Pid::new(target_pid as usize);

    if hyperion_scheduler::send(pid, data.to_vec().into()).is_err() {
        return -3;
    }

    return 0;
}

/// recv data from this process input channel
///
/// # arguments
///  - `syscall_id` : 12
///  - `arg0`       : data ptr
///  - `arg1`       : data len (bytes)
///
/// # return codes (in syscall_id after returning)
///  - `-2` : address range not mapped for the user (arg0 .. arg1)
///  - `-1` : invalid address range (arg0 .. arg1)
///  - `0..` : num of bytes read
pub fn recv(args: &mut SyscallRegs) -> i64 {
    let buf = match read_untrusted_bytes_mut(args.arg0, args.arg1) {
        Ok(v) => v,
        Err(err) => {
            return err;
        }
    };

    return hyperion_scheduler::recv_to(buf) as i64;
}

//

fn read_slice_parts(ptr: u64, len: u64) -> Result<(VirtAddr, usize), i64> {
    let Some(end) = ptr.checked_add(len) else {
        return Err(-1);
    };

    let (Ok(start), Ok(end)) = (VirtAddr::try_new(ptr), VirtAddr::try_new(end)) else {
        return Err(-1);
    };

    if !PageMap::current().is_mapped(start..end, PageTableFlags::USER_ACCESSIBLE) {
        return Err(-2);
    }

    Ok((start, len as _))
}

fn read_untrusted_bytes<'a>(ptr: u64, len: u64) -> Result<&'a [u8], i64> {
    read_slice_parts(ptr, len).map(|(start, len)| {
        // TODO:
        // SAFETY: this is most likely unsafe
        unsafe { core::slice::from_raw_parts(start.as_ptr(), len as _) }
    })
}

fn read_untrusted_bytes_mut<'a>(ptr: u64, len: u64) -> Result<&'a mut [u8], i64> {
    read_slice_parts(ptr, len).map(|(start, len)| {
        // TODO:
        // SAFETY: this is most likely unsafe
        unsafe { core::slice::from_raw_parts_mut(start.as_mut_ptr(), len as _) }
    })
}

fn read_untrusted_str<'a>(ptr: u64, len: u64) -> Result<&'a str, i64> {
    core::str::from_utf8(read_untrusted_bytes(ptr, len)?).map_err(|_| -3)
}
