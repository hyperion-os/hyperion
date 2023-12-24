#![no_std]

use core::{
    ptr::{self, NonNull},
    sync::atomic::AtomicUsize,
};

use err::Result;

use self::{
    fs::{FileDesc, FileOpenFlags, Metadata},
    net::{Protocol, SocketDesc, SocketDomain, SocketType},
};

//

pub mod err;
pub mod fs;
pub mod net;

pub mod id {
    pub const LOG: usize = 1;
    pub const EXIT: usize = 420;
    pub const DONE: usize = 421;
    pub const YIELD_NOW: usize = 3;
    pub const TIMESTAMP: usize = 4;
    pub const NANOSLEEP: usize = 5;
    pub const NANOSLEEP_UNTIL: usize = 6;

    pub const SPAWN: usize = 8;
    pub const PALLOC: usize = 9;
    pub const PFREE: usize = 10;
    pub const SEND: usize = 11;
    pub const RECV: usize = 12;
    pub const RENAME: usize = 13;

    pub const OPEN: usize = 14;
    pub const CLOSE: usize = 15;
    pub const READ: usize = 16;
    pub const WRITE: usize = 17;

    pub const SOCKET: usize = 18;
    pub const BIND: usize = 19;
    pub const LISTEN: usize = 20;
    pub const ACCEPT: usize = 21;
    pub const CONNECT: usize = 22;

    pub const GET_PID: usize = 23;
    pub const GET_TID: usize = 24;

    pub const DUP: usize = 25;
    pub const OPEN_DIR: usize = 26;
    pub const FUTEX_WAIT: usize = 27;
    pub const FUTEX_WAKE: usize = 28;

    pub const MAP_FILE: usize = 29;
    pub const UNMAP_FILE: usize = 30;
    pub const METADATA: usize = 31;
    pub const SEEK: usize = 32;
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
                mut $id: usize
                $(, $a0: usize $(, $a1: usize $(, $a2: usize $(, $a3: usize $(, $a4: usize)?)?)?)?)?
            ) -> $crate::err::Result<usize> {
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

//

/// print a string into kernel logs
#[inline(always)]
pub fn log(str: &str) -> Result<()> {
    // TODO: should null terminated strings be used instead to save registers?
    // decide later™

    unsafe { syscall_2(id::LOG, str.as_ptr() as usize, str.len()) }.map(|_| {})
}

/// exit the process with a code
#[inline(always)]
pub fn exit(code: i64) -> ! {
    let result = unsafe { syscall_1(id::EXIT, code as usize) };
    unreachable!("{result:?}");
}

/// exit the thread with a code
#[inline(always)]
pub fn done(code: i64) -> ! {
    let result = unsafe { syscall_1(id::DONE, code as usize) };
    unreachable!("{result:?}");
}

/// context switch from this process, no guarantees about actually switching
#[inline(always)]
pub fn yield_now() {
    unsafe { syscall_0(id::YIELD_NOW) }.unwrap();
}

/// u128 nanoseconds since boot
#[inline(always)]
pub fn timestamp() -> Result<u128> {
    let mut result: u128 = 0;
    unsafe { syscall_1(id::TIMESTAMP, &mut result as *mut u128 as usize) }.map(move |_| result)
}

/// context switch from this process and switch back when `nanos` nanoseconds have passed
#[inline(always)]
pub fn nanosleep(nanos: u64) {
    // TODO: u128
    unsafe { syscall_1(id::NANOSLEEP, nanos as usize) }.unwrap();
}

/// context switch from this process and switch back when [`timestamp()`] > `deadline_nanos`
///
/// might not happen immediately when it is true
#[inline(always)]
pub fn nanosleep_until(deadline_nanos: u64) {
    // TODO: u128
    unsafe { syscall_1(id::NANOSLEEP_UNTIL, deadline_nanos as usize) }.unwrap();
}

/// spawn a new pthread for the same process
#[inline(always)]
pub fn spawn(thread_entry: extern "C" fn(usize, usize) -> !, arg: usize) {
    unsafe { syscall_2(id::SPAWN, thread_entry as usize, arg) }.unwrap();
}

/// allocate physical pages and map to heap
#[inline(always)]
pub fn palloc(pages: usize) -> Result<Option<NonNull<u8>>> {
    unsafe { syscall_1(id::PALLOC, pages) }.map(|ptr| NonNull::new(ptr as _))
}

/// deallocate physical pages and unmap from heap
#[inline(always)]
pub fn pfree(ptr: NonNull<u8>, pages: usize) -> Result<()> {
    unsafe { syscall_2(id::PFREE, ptr.as_ptr() as usize, pages) }.map(|_| {})
}

/// rename the current process
#[inline(always)]
pub fn rename(new_name: &str) -> Result<()> {
    unsafe { syscall_2(id::RENAME, new_name.as_ptr() as usize, new_name.len()) }.map(|_| {})
}

/// open a file
#[inline(always)]
pub fn open(path: &str, flags: FileOpenFlags, mode: usize) -> Result<FileDesc> {
    unsafe {
        syscall_4(
            id::OPEN,
            path.as_ptr() as usize,
            path.len(),
            flags.bits(),
            mode,
        )
    }
    .map(FileDesc)
}

/// close a file
#[inline(always)]
pub fn close(file: FileDesc) -> Result<()> {
    unsafe { syscall_1(id::CLOSE, file.0) }.map(|_| {})
}

/// read from a file
#[inline(always)]
pub fn read(file: FileDesc, buf: &mut [u8]) -> Result<usize> {
    unsafe { syscall_3(id::READ, file.0, buf.as_mut_ptr() as usize, buf.len()) }
}

/// write into a file
#[inline(always)]
pub fn write(file: FileDesc, buf: &[u8]) -> Result<usize> {
    unsafe { syscall_3(id::WRITE, file.0, buf.as_ptr() as usize, buf.len()) }
}

/// create a socket
#[inline(always)]
pub fn socket(domain: SocketDomain, ty: SocketType, protocol: Protocol) -> Result<SocketDesc> {
    unsafe { syscall_3(id::SOCKET, domain.0, ty.0, protocol.0) }.map(SocketDesc)
}

/// bind a name to a socket
#[inline(always)]
pub fn bind(socket: SocketDesc, addr: &str) -> Result<()> {
    unsafe { syscall_3(id::BIND, socket.0, addr.as_ptr() as _, addr.len()) }.map(|_| {})
}

/// start listening for connections on a socket
#[inline(always)]
pub fn listen(socket: SocketDesc) -> Result<()> {
    unsafe { syscall_1(id::LISTEN, socket.0) }.map(|_| {})
}

/// accept a connection on a socket
#[inline(always)]
pub fn accept(socket: SocketDesc) -> Result<SocketDesc> {
    unsafe { syscall_1(id::ACCEPT, socket.0) }.map(SocketDesc)
}

/// connect to a socket
#[inline(always)]
pub fn connect(socket: SocketDesc, addr: &str) -> Result<()> {
    unsafe { syscall_3(id::CONNECT, socket.0, addr.as_ptr() as _, addr.len()) }.map(|_| {})
}

/// send data to a socket
#[inline(always)]
pub fn send(socket: SocketDesc, data: &[u8], flags: usize) -> Result<()> {
    let (data, data_len) = (data.as_ptr() as usize, data.len());
    unsafe { syscall_4(id::SEND, socket.0, data, data_len, flags) }.map(|_| {})
}

/// read data from a socket
pub fn recv(socket: SocketDesc, buf: &mut [u8], flags: usize) -> Result<usize> {
    let (buf, buf_len) = (buf.as_ptr() as usize, buf.len());
    unsafe { syscall_4(id::RECV, socket.0, buf, buf_len, flags) }
}

/// get the current process id
#[inline(always)]
pub fn get_pid() -> usize {
    unsafe { syscall_0(id::GET_PID) }.unwrap()
}

/// get the current thread id
#[inline(always)]
pub fn get_tid() -> usize {
    unsafe { syscall_0(id::GET_TID) }.unwrap()
}

/// duplicate a file descriptor
#[inline(always)]
pub fn dup(old: FileDesc, new: FileDesc) -> Result<FileDesc> {
    unsafe { syscall_2(id::DUP, old.0, new.0) }.map(FileDesc)
}

/// open a directory
#[inline(always)]
pub fn open_dir(path: &str) -> Result<FileDesc> {
    unsafe { syscall_2(id::OPEN_DIR, path.as_ptr() as _, path.len()) }.map(FileDesc)
}

/// futex wait if value at `addr` is `val`
///
/// wakes up when some other thread calls `futex_wake` on the same `addr`
///
/// the addr is translated so futexes in inter-process shmem should still work
#[inline(always)]
pub fn futex_wait(addr: &AtomicUsize, val: usize) {
    unsafe { syscall_2(id::FUTEX_WAIT, addr as *const _ as usize, val) }.unwrap();
}

/// wake `num` threads that are sleeping on this `addr`
///
/// see [`futex_wait`]
#[inline(always)]
pub fn futex_wake(addr: &AtomicUsize, num: usize) {
    unsafe { syscall_2(id::FUTEX_WAKE, addr as *const _ as usize, num) }.unwrap();
}

/// map file contents to memory (mmap)
///
/// maps pages from the file at `align_down(offset, 0x1000)..align_up(offset+size, 0x1000)`
/// to the virtual address space at `align_down(at, 0x1000)`, or anywhere if `at` is None
///
/// `at` should point to unmapped memory that has room for the pages
#[inline(always)]
pub fn map_file(
    file: FileDesc,
    at: Option<NonNull<()>>,
    size: usize,
    offset: usize,
) -> Result<NonNull<()>> {
    let at = at.map(NonNull::as_ptr).unwrap_or(ptr::null_mut()) as usize;
    unsafe { syscall_4(id::MAP_FILE, file.0, at, size, offset) }
        .map(|ptr| NonNull::new(ptr as _).unwrap())
}

/// unmap device/file mapped memory (munmap)
///
/// see [`unmap_file`]
#[inline(always)]
pub fn unmap_file(file: FileDesc, at: NonNull<()>, size: usize) -> Result<()> {
    let at = at.as_ptr() as usize;
    unsafe { syscall_3(id::UNMAP_FILE, file.0, at, size) }.map(|_| {})
}

/// file metadata (stat)
#[inline(always)]
pub fn metadata(file: FileDesc, metadata: &mut Metadata) -> Result<()> {
    unsafe { syscall_2(id::METADATA, file.0, metadata as *mut _ as usize) }.map(|_| {})
}

/// file position seek (fseek)
#[inline(always)]
pub fn seek(file: FileDesc, offset: isize, origin: usize) -> Result<()> {
    unsafe { syscall_3(id::SEEK, file.0, offset as _, origin) }.map(|_| {})
}
