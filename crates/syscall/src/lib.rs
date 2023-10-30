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
pub fn log(str: &str) -> Result<(), u64> {
    // TODO: should null terminated strings be used instead to save registers?
    // decide later™

    let result: u64;
    unsafe { syscall!(1, { str.as_ptr() as u64, str.len() }, { result }) };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

/// exit the process with a code
#[inline(always)]
pub fn exit(code: i64) -> ! {
    unsafe { syscall!(2, { code as u64 }, {}) };
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
pub fn nanosleep(nanos: u64) {
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
pub fn pthread_spawn(thread_entry: extern "C" fn(u64, u64) -> !, arg: u64) {
    unsafe { syscall!(8, { thread_entry as usize as u64, arg }, {}) };
}

/// allocate physical pages and map to heap
#[inline(always)]
pub fn palloc(pages: u64) -> Result<*mut u8, i64> {
    let result: i64;
    unsafe { syscall!(9, { pages }, { result }) };
    if result >= 0 {
        Ok(result as _)
    } else {
        Err(result)
    }
}

/// deallocate physical pages and unmap from heap
#[inline(always)]
pub fn pfree(ptr: u64, pages: u64) -> Result<(), i64> {
    let result: i64;
    unsafe { syscall!(10, { ptr, pages }, { result }) };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

/// send data to a PID based single naïve IPC channel
#[inline(always)]
pub fn send(target: u64, data: &[u8]) -> Result<(), i64> {
    let result: i64;
    unsafe { syscall!(11, { target, data.as_ptr() as u64, data.len() as u64 }, { result }) };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

/// read data from a PID based single naïve IPC channel
pub fn recv(buf: &mut [u8]) -> Result<u64, i64> {
    let result: i64;
    unsafe { syscall!(12, { buf.as_mut_ptr() as u64, buf.len() as u64 }, { result }) };
    if result >= 0 {
        Ok(result as _)
    } else {
        Err(result)
    }
}

/// rename the current process
#[inline(always)]
pub fn rename(new_name: &str) -> Result<(), i64> {
    let result: i64;
    unsafe { syscall!(13, { new_name.as_ptr() as u64, new_name.len() as u64 }, { result }) };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}
