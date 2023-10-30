#![no_std]

//

macro_rules! syscall {
    ($id:literal,
     // wtf:
     { $($arg0:expr $(,$arg1:expr $(,$arg2:expr $(,$arg3:expr $(,$arg4:expr $(,)?)?)?)?)?)? },
     { $($ret0:expr $(,$ret1:expr $(,$ret2:expr $(,$ret3:expr $(,$ret4:expr $(,)?)?)?)?)?)? }) => {


        core::arch::asm!(
            "syscall",
            in("rax") $id,

            $(
                in("rdi") $arg0,
            $(
                in("rsi") $arg1,
            $(
                in("rdx") $arg2,
            $(
                in("r8")  $arg3,
            $(
                in("r9")  $arg4,
            )?)?)?)?)?

            $(
                lateout("rdi") $ret0,
            $(
                lateout("rsi") $ret1,
            $(
                lateout("rdx") $ret2,
            $(
                lateout("r8")  $ret3,
            $(
                lateout("r9")  $ret4,
            )?)?)?)?)?
        )
    };
}

//

/// print a string into kernel logs
#[inline(always)]
pub fn log(str: &str) -> usize {
    // TODO: should null terminated strings be used instead to save registers?
    // decide later™

    let result: usize;
    unsafe { syscall!(1, { str.as_ptr() as usize, str.len() }, { result }) };
    result
}

/// exit the process with a code
#[inline(always)]
pub fn exit(code: isize) -> ! {
    unsafe { syscall!(2, { code as usize }, {}) };
    unreachable!();
}

/// context switch from this process, no guarantees about actually switching
#[inline(always)]
pub fn yield_now() {
    unsafe { syscall!(3, {}, {}) };
}

/// u128 nanoseconds since boot
#[inline(always)]
pub fn timestamp() -> Result<u128, u64> {
    let result: u64;
    let lower: u64;
    let upper: u64;
    unsafe { syscall!(4, {}, { result, lower, upper }) };

    if result == 0 {
        Ok(lower as u128 | (upper as u128) << 64)
    } else {
        Err(result)
    }
}

/// context switch from this process and switch back when `nanos` nanoseconds have passed
#[inline(always)]
pub fn nanosleep(nanos: usize) {
    unsafe { syscall!(5, { nanos }, {}) };
}

/// context switch from this process and switch back when [`timestamp()`] > `deadline_nanos`
///
/// might not happen immediately when it is true
#[inline(always)]
pub fn nanosleep_until(deadline_nanos: u64) {
    unsafe { syscall!(6, { deadline_nanos }, {}) };
}

/// spawn a new pthread for the same process
#[inline(always)]
pub fn pthread_spawn(thread_entry: extern "C" fn(usize, usize) -> !, arg: usize) {
    unsafe { syscall!(8, { thread_entry as usize, arg }, {}) };
}

/// allocate physical pages and map to heap
#[inline(always)]
pub fn palloc(pages: usize) -> Result<*mut (), usize> {
    unsafe { syscall!(9, { pages }, {}) };
}

/// deallocate physical pages and unmap from heap
#[inline(always)]
pub fn pfree(ptr: u64, pages: u64) -> i64 {
    unsafe { trigger_syscall(10, ptr, pages, 0, 0, 0) as i64 }
}

/// send data to a PID based single naïve IPC channel
#[inline(always)]
pub fn send(target: u64, data: &[u8]) -> i64 {
    unsafe { trigger_syscall(11, target, data.as_ptr() as u64, data.len() as u64, 0, 0) as i64 }
}

/// read data from a PID based single naïve IPC channel
#[inline(always)]
pub fn recv(buf: &mut [u8]) -> i64 {
    unsafe { trigger_syscall(12, buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0, 0) as i64 }
}

/// # Safety
/// the `syscall_id` and its arguments have to be valid or this program could accidentally close
/// itself or share its memory or something
#[inline(always)]
pub unsafe extern "C" fn trigger_syscall_0_1(
    syscall_id: usize,
    // arg0: u64,
    // arg1: u64,
    // arg2: u64,
    // arg3: u64,
    // arg4: u64,
) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") syscall_id,
            // in("rdi") arg0,
            // in("rsi") arg1,
            // in("rdx") arg2,
            // in("r8") arg3,
            // in("r9") arg4,
            lateout("rax") result,
        );
    }
    result
}

/// # Safety
/// the `syscall_id` and its arguments have to be valid or this program could accidentally close
/// itself or share its memory or something
#[inline(always)]
pub unsafe extern "C" fn trigger_syscall_1(
    syscall_id: usize,
    arg0: usize,
    // arg1: u64,
    // arg2: u64,
    // arg3: u64,
    // arg4: u64,
) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") syscall_id,
            in("rdi") arg0,
            // in("rsi") arg1,
            // in("rdx") arg2,
            // in("r8") arg3,
            // in("r9") arg4,
            lateout("rax") result
        );
    }
    result
}

/* #[inline(always)]
pub extern "C" fn trigger_syscall(
    _rdi: u64,
    _rsi: u64,
    _rdx: u64,
    _rcx_ignored: u64,
    _r8: u64,
    _r9: u64,
) {
    unsafe {
        core::arch::asm!("syscall");
    }
} */
