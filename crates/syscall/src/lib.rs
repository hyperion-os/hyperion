#![no_std]

//

use core::{
    fmt::{self, Arguments, Write},
    mem::MaybeUninit,
    ptr::{self, NonNull},
    sync::atomic::AtomicUsize,
};

use bitflags::bitflags;
use err::Result;

use crate::{
    fs::{FileDesc, FileOpenFlags, Metadata},
    net::{Protocol, SocketDomain, SocketType},
};

//

pub mod err;
pub mod fs;
pub mod net;

#[cfg(feature = "rustc-dep-of-std")]
pub mod libc;

//

macro_rules! impl_try_into {
    (pub enum Id {
        $($variant:ident = $id:literal),* $(,)?
    }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        #[repr(usize)]
        pub enum Id {
            $($variant = $id,)*
        }

        impl TryFrom<usize> for Id {
            type Error = InvalidSyscall;

            fn try_from(value: usize) -> Result<Self, Self::Error> {
                match value {
                    $($id => Ok(Id::$variant),)*
                    _ => Err(InvalidSyscall),
                }
            }
        }
    };
}

impl_try_into! {
pub enum Id {
    Log = 1,
    Exit = 420,
    Done = 421,
    YieldNow = 3,
    Timestamp = 4,
    Nanosleep = 5,
    NanosleepUntil = 6,

    Spawn = 8,
    Send = 11,
    Recv = 12,
    Rename = 13,

    Open = 14,
    Close = 15,
    Read = 16,
    Write = 17,

    Socket = 18,
    Bind = 19,
    Listen = 20,
    Accept = 21,
    Connect = 22,

    GetPid = 23,
    GetTid = 24,

    Dup = 25,
    Pipe = 26,
    FutexWait = 27,
    FutexWake = 28,

    MemMap = 29,
    MemUnmap = 30,
    Metadata = 31,
    Seek = 32,

    System = 33,
    Fork = 34,
    Waitpid = 35,
}
}

#[derive(Debug, Clone, Copy)]
pub struct InvalidSyscall;

impl fmt::Display for InvalidSyscall {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("invalid syscall")
    }
}

//

// TODO: fork and exec
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LaunchConfig {
    pub stdin: FileDesc,
    pub stdout: FileDesc,
    pub stderr: FileDesc,
}

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
                $id: Id
                $(, $a0: usize $(, $a1: usize $(, $a2: usize $(, $a3: usize $(, $a4: usize)?)?)?)?)?
            ) -> $crate::err::Result<usize> {
                let mut $id = $id as usize;

                unsafe { core::arch::asm!(
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
                ) };

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

#[macro_export]
macro_rules! log {
    ($($t:tt)*) => {
        $crate::_sys_log(format_args!("{}\n", format_args!($($t)*)));
    };
}

//

#[doc(hidden)]
pub fn _sys_log(args: Arguments) {
    struct SysLog;

    impl Write for SysLog {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            log(s).map_err(|_| fmt::Error)
        }
    }

    _ = SysLog.write_fmt(args);
}

//

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct MemMapFlags: u32 {
        /// shared memory
        ///
        /// updates to the mapping are visible to
        /// other processes that mapped the same file
        const SHARED  = 0b0100_0000;
        /// copy-on-write
        ///
        /// updates to the mapping are not visible to
        /// other processes that mapped the same file,
        /// instead a copy of the contents are made
        const PRIVATE = 0b0010_0000;
        /// ram (instead of fd)
        ///
        /// doesnt use a file, but maps normal memory
        /// that is lazy allocated and uninitialized (usually zeroed)
        const ANON    = 0b0001_0000;
        /// the addr hint isn't a hint but an exact address,
        /// overlapping part of previous mappings get discarded
        ///
        /// mem_map doesn't overwrite old mappings without this (thats a lie)
        const FIXED   = 0b0000_1000;

        /// allows executing
        const EXEC    = 0b0000_0100;
        /// allows reading
        const READ    = 0b0000_0010;
        /// allows writing
        const WRITE   = 0b0000_0001;

        /// just an alias for read+write+exec
        const RWE     = Self::READ.bits() | Self::WRITE.bits() | Self::EXEC.bits();
        /// just an alias for read+write
        const RW      = Self::READ.bits() | Self::WRITE.bits();
        /// just an alias for read+write+anon
        const HEAP    = Self::READ.bits() | Self::WRITE.bits() | Self::ANON.bits();
    }
}

//

/// print a string into kernel logs
pub fn log(str: &str) -> Result<()> {
    // TODO: should null terminated strings be used instead to save registers?
    // decide later™

    unsafe { syscall_2(Id::Log, str.as_ptr() as usize, str.len()) }.map(|_| {})
}

/// exit the process with a code
pub fn exit(code: i64) -> ! {
    let result = unsafe { syscall_1(Id::Exit, code as usize) };
    unreachable!("{result:?}");
}

/// exit the thread with a code
pub fn done(code: i64) -> ! {
    let result = unsafe { syscall_1(Id::Done, code as usize) };
    unreachable!("{result:?}");
}

/// context switch from this process, no guarantees about actually switching
pub fn yield_now() {
    _ = unsafe { syscall_0(Id::YieldNow) };
}

/// u128 nanoseconds since boot
pub fn timestamp() -> Result<u128> {
    let mut result: u128 = 0;
    unsafe { syscall_1(Id::Timestamp, core::ptr::addr_of_mut!(result) as usize) }
        .map(move |_| result)
}

/// context switch from this process and switch back when `nanos` nanoseconds have passed
pub fn nanosleep(nanos: u64) {
    // TODO: u128
    unsafe { syscall_1(Id::Nanosleep, nanos as usize) }.unwrap();
}

/// context switch from this process and switch back when [`timestamp()`] > `deadline_nanos`
///
/// might not happen immediately when it is true
pub fn nanosleep_until(deadline_nanos: u64) {
    // TODO: u128
    unsafe { syscall_1(Id::NanosleepUntil, deadline_nanos as usize) }.unwrap();
}

/// spawn a new pthread for the same process
pub fn spawn(ip: extern "C" fn() -> !, sp: usize) {
    unsafe { syscall_2(Id::Spawn, ip as usize, sp) }.unwrap();
}

/// rename the current process
pub fn rename(new_name: &str) -> Result<()> {
    unsafe { syscall_2(Id::Rename, new_name.as_ptr() as usize, new_name.len()) }.map(|_| {})
}

/// open a file
pub fn open(path: &str, flags: FileOpenFlags, mode: usize) -> Result<FileDesc> {
    unsafe {
        syscall_4(
            Id::Open,
            path.as_ptr() as usize,
            path.len(),
            flags.bits(),
            mode,
        )
    }
    .map(FileDesc)
}

/// close a file
pub fn close(file: FileDesc) -> Result<()> {
    unsafe { syscall_1(Id::Close, file.0) }.map(|_| {})
}

/// read from a file
pub fn read(file: FileDesc, buf: &mut [u8]) -> Result<usize> {
    unsafe { syscall_3(Id::Read, file.0, buf.as_mut_ptr() as usize, buf.len()) }
}

/// read from a file
pub fn read_uninit(file: FileDesc, buf: &mut [MaybeUninit<u8>]) -> Result<usize> {
    unsafe { syscall_3(Id::Read, file.0, buf.as_mut_ptr() as usize, buf.len()) }
}

/// write into a file
pub fn write(file: FileDesc, buf: &[u8]) -> Result<usize> {
    unsafe { syscall_3(Id::Write, file.0, buf.as_ptr() as usize, buf.len()) }
}

/// create a socket
pub fn socket(domain: SocketDomain, ty: SocketType, protocol: Protocol) -> Result<FileDesc> {
    unsafe { syscall_3(Id::Socket, domain.0, ty.0, protocol.0) }.map(FileDesc)
}

/// bind a name to a socket
pub fn bind(socket: FileDesc, addr: &str) -> Result<()> {
    unsafe { syscall_3(Id::Bind, socket.0, addr.as_ptr() as _, addr.len()) }.map(|_| {})
}

/// start listening for connections on a socket
pub fn listen(socket: FileDesc) -> Result<()> {
    unsafe { syscall_1(Id::Listen, socket.0) }.map(|_| {})
}

/// accept a connection on a socket
pub fn accept(socket: FileDesc) -> Result<FileDesc> {
    unsafe { syscall_1(Id::Accept, socket.0) }.map(FileDesc)
}

/// connect to a socket
pub fn connect(socket: FileDesc, addr: &str) -> Result<()> {
    unsafe { syscall_3(Id::Connect, socket.0, addr.as_ptr() as _, addr.len()) }.map(|_| {})
}

/// send data to a socket
pub fn send(socket: FileDesc, data: &[u8], flags: usize) -> Result<usize> {
    let (data, data_len) = (data.as_ptr() as usize, data.len());
    unsafe { syscall_4(Id::Send, socket.0, data, data_len, flags) }
}

/// read data from a socket
pub fn recv(socket: FileDesc, buf: &mut [u8], flags: usize) -> Result<usize> {
    let (buf, buf_len) = (buf.as_ptr() as usize, buf.len());
    unsafe { syscall_4(Id::Recv, socket.0, buf, buf_len, flags) }
}

/// get the current process id
#[must_use]
pub fn get_pid() -> usize {
    // SAFETY: this syscall cannot fail, look at the source
    unsafe { syscall_0(Id::GetPid).unwrap_unchecked() }
}

/// get the current thread id
#[must_use]
pub fn get_tid() -> usize {
    // SAFETY: this syscall cannot fail, look at the source
    unsafe { syscall_0(Id::GetTid).unwrap_unchecked() }
}

/// duplicate a file descriptor
pub fn dup(old: FileDesc, new: FileDesc) -> Result<FileDesc> {
    unsafe { syscall_2(Id::Dup, old.0, new.0) }.map(FileDesc)
}

/// create a new pipe
pub fn pipe() -> Result<[FileDesc; 2]> {
    let mut pipes = [FileDesc(0); 2];
    unsafe { syscall_1(Id::Pipe, pipes.as_mut_ptr() as usize) }?;
    Ok(pipes)
}

/// futex wait if value at `addr` is `val`
///
/// wakes up when some other thread calls `futex_wake` on the same `addr`
///
/// the addr is translated so futexes in inter-process shmem should still work
pub fn futex_wait(addr: &AtomicUsize, val: usize) {
    unsafe { syscall_2(Id::FutexWait, addr as *const _ as usize, val) }.unwrap();
}

/// wake `num` threads that are sleeping on this `addr`
///
/// see [`futex_wait`]
pub fn futex_wake(addr: &AtomicUsize, num: usize) {
    unsafe { syscall_2(Id::FutexWake, addr as *const _ as usize, num) }.unwrap();
}

/// map file contents to memory (mmap)
///
/// maps pages from the file at `align_down(offset, 0x1000)..align_up(offset+size, 0x1000)`
/// to the virtual address space at `align_down(at, 0x1000)`, or anywhere if `at` is None
///
/// `at` should point to unmapped memory that has room for the pages
pub fn mem_map(
    addr: Option<NonNull<()>>,
    size: usize,
    flags: MemMapFlags,
    fd: FileDesc,
    offset: usize,
) -> Result<NonNull<()>> {
    let at = addr.map_or(ptr::null_mut(), NonNull::as_ptr) as usize;
    unsafe { syscall_5(Id::MemMap, at, size, flags.bits() as _, fd.0, offset) }
        .map(|ptr| NonNull::new(ptr as _).unwrap())
}

/// unmap device/file mapped memory (munmap)
pub fn mem_unmap(addr: NonNull<()>, size: usize) -> Result<()> {
    unsafe { syscall_2(Id::MemUnmap, addr.as_ptr() as usize, size) }.map(|_| {})
}

/// file metadata (stat)
pub fn metadata(file: FileDesc, metadata: &mut Metadata) -> Result<()> {
    unsafe { syscall_2(Id::Metadata, file.0, metadata as *mut _ as usize) }.map(|_| {})
}

/// file position seek (fseek)
pub fn seek(file: FileDesc, offset: isize, origin: usize) -> Result<()> {
    unsafe { syscall_3(Id::Seek, file.0, offset as _, origin) }.map(|_| {})
}

/// launch a process
pub fn system(path: &str, args: &[&str]) -> Result<usize> {
    unsafe {
        syscall_5(
            Id::System,
            path.as_ptr() as usize,
            path.len(),
            args.as_ptr() as usize,
            args.len(),
            0,
        )
    }
}

/// launch a process with config
pub fn system_with(path: &str, args: &[&str], cfg: LaunchConfig) -> Result<usize> {
    unsafe {
        syscall_5(
            Id::System,
            path.as_ptr() as usize,
            path.len(),
            args.as_ptr() as usize,
            args.len(),
            &cfg as *const LaunchConfig as usize,
        )
    }
}

/// fork the current process and return the PID
pub fn fork() -> usize {
    unsafe { syscall_0(Id::Fork) }.unwrap()
}

/// wait for a PID to exit
/// TODO: this should be like https://linux.die.net/man/2/waitpid in the future
pub fn waitpid(pid: usize) -> usize {
    unsafe { syscall_1(Id::Waitpid, pid) }.unwrap()
}
