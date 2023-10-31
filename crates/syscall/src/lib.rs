#![no_std]

use core::ptr::NonNull;

use err::Result;

//

pub mod err;

//

macro_rules! syscall {
    (
        $(
            $name:ident(
                $id:ident
                $(, $a0:ident $(, $a1:ident $(, $a2:ident $(, $a3:ident $(, $a4:ident)?)?)?)?)?
            );
        )+
    ) => {
        $(
            /// # Safety
            /// TODO:
            /// invalid syscall args can terminate this process
            pub unsafe fn $name(
                mut $id: usize
                $(, $a0: usize $(, $a1: usize $(, $a2: usize $(, $a3: usize $(, $a4: usize)?)?)?)?)?
            ) -> $crate::err::Result<usize> {
                core::arch::asm!(
                    "syscall",

                    inout("rax") $id, // syscall id + return value

                    $( // optional args
                        in("rdi") $a0,
                        $(
                            in("rsi") $a1,
                            $(
                                in("rdx") $a2,
                                $(
                                    in("r8") $a3,
                                    $(
                                        in("r9") $a4,
                                    )?
                                )?
                            )?
                        )?
                    )?

                    out("rcx") _, // remind the compiler that
                    out("r11") _, // syscall saves these 2

                    options(nostack),
                );

                $crate::err::Error::decode($id)
            }
        )+
    };
}

syscall! {
    syscall_0(syscall_id);
    syscall_1(syscall_id, a0);
    syscall_2(syscall_id, a0, a1);
    syscall_3(syscall_id, a0, a1, a2);
    syscall_4(syscall_id, a0, a1, a2, a3);
    syscall_5(syscall_id, a0, a1, a2, a3, a4);
}

//

/// print a string into kernel logs
#[inline(always)]
pub fn log(str: &str) -> Result<()> {
    // TODO: should null terminated strings be used instead to save registers?
    // decide later™

    unsafe { syscall_2(1, str.as_ptr() as usize, str.len()) }.map(|_| {})
}

/// exit the process with a code
#[inline(always)]
pub fn exit(code: i64) -> ! {
    let result = unsafe { syscall_1(2, code as usize) };
    unreachable!("{result:?}");
}

/// context switch from this process, no guarantees about actually switching
#[inline(always)]
pub fn yield_now() {
    unsafe { syscall_0(3) }.unwrap();
}

/// u128 nanoseconds since boot
#[inline(always)]
pub fn timestamp() -> Result<u128> {
    let mut result: u128 = 0;
    unsafe { syscall_1(4, &mut result as *mut u128 as usize) }.map(move |_| result)
}

/// context switch from this process and switch back when `nanos` nanoseconds have passed
#[inline(always)]
pub fn nanosleep(nanos: u64) {
    // TODO: u128
    unsafe { syscall_1(5, nanos as usize) }.unwrap();
}

/// context switch from this process and switch back when [`timestamp()`] > `deadline_nanos`
///
/// might not happen immediately when it is true
#[inline(always)]
pub fn nanosleep_until(deadline_nanos: u64) {
    // TODO: u128
    unsafe { syscall_1(6, deadline_nanos as usize) }.unwrap();
}

/// spawn a new pthread for the same process
#[inline(always)]
pub fn pthread_spawn(thread_entry: extern "C" fn(usize, usize) -> !, arg: usize) {
    unsafe { syscall_2(8, thread_entry as usize, arg) }.unwrap();
}

/// allocate physical pages and map to heap
#[inline(always)]
pub fn palloc(pages: usize) -> Result<Option<NonNull<u8>>> {
    unsafe { syscall_1(9, pages) }.map(|ptr| NonNull::new(ptr as _))
}

/// deallocate physical pages and unmap from heap
#[inline(always)]
pub fn pfree(ptr: NonNull<u8>, pages: usize) -> Result<()> {
    unsafe { syscall_2(10, ptr.as_ptr() as usize, pages) }.map(|_| {})
}

/// send data to a PID based single naïve IPC channel
#[inline(always)]
pub fn send(target: u64, data: &[u8]) -> Result<()> {
    unsafe { syscall_3(11, target as usize, data.as_ptr() as usize, data.len()) }.map(|_| {})
}

/// read data from a PID based single naïve IPC channel
pub fn recv(buf: &mut [u8]) -> Result<usize> {
    unsafe { syscall_2(12, buf.as_mut_ptr() as usize, buf.len()) }
}

/// rename the current process
#[inline(always)]
pub fn rename(new_name: &str) -> Result<()> {
    unsafe { syscall_2(13, new_name.as_ptr() as usize, new_name.len()) }.map(|_| {})
}
