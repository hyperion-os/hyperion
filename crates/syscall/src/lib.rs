#![no_std]

#[inline(always)]
pub fn log(str: &str) -> u64 {
    unsafe {
        // TODO: should null terminated strings be used instead to save registers?
        // decide later™
        trigger_syscall(1, str.as_ptr() as u64, str.len() as u64, 0, 0, 0)
    }
}

#[inline(always)]
pub fn exit(code: i64) -> ! {
    unsafe {
        trigger_syscall(2, code as u64, 0, 0, 0, 0);
    }

    unreachable!();
}

#[inline(always)]
pub fn yield_now() {
    unsafe {
        trigger_syscall(3, 0, 0, 0, 0, 0);
    }
}

#[inline(always)]
pub fn commit_oxygen_not_reach_lungs(code: i64) -> ! {
    unsafe {
        trigger_syscall(420, code as u64, 0, 0, 0, 0);
    }

    unreachable!();
}

/// # Safety
/// the `syscall_id` and its arguments have to be valid or this program could accidentally close
/// itself or share its memory or something
#[inline(always)]
pub unsafe extern "C" fn trigger_syscall(
    syscall_id: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
) -> u64 {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") syscall_id,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            in("r8") arg3,
            in("r9") arg4,
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
