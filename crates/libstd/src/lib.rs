#![no_std]
#![feature(format_args_nl)]

//

extern crate alloc;

use core::{
    alloc::GlobalAlloc,
    fmt::{self, Write},
    ptr::NonNull,
};

use hyperion_syscall::{exit, palloc, pfree};

//

pub mod sys {
    pub use hyperion_syscall::*;
}

pub mod fs;

pub mod thread;

//

#[macro_export]
macro_rules! println {
    ($($v:tt)*) => {
        $crate::_print(format_args_nl!($($v)*))
    };
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    struct SyscallLog;

    //

    impl Write for SyscallLog {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            hyperion_syscall::log(s).map_err(|_| fmt::Error)
        }
    }

    _ = SyscallLog.write_fmt(args);
}

//

#[no_mangle]
extern "C" fn _start(a0: usize) -> ! {
    extern "Rust" {
        fn main(a: CliArgs);
    }

    unsafe {
        main(CliArgs {
            hyperion_cli_args_ptr: a0,
        });
    }

    exit(0);
}

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    println!("{info}");
    exit(-1);
}

//

#[derive(Clone, Copy)]
pub struct CliArgs {
    hyperion_cli_args_ptr: usize,
}

impl CliArgs {
    pub fn iter(self) -> impl Iterator<Item = &'static str> + Clone + DoubleEndedIterator {
        let mut ptr = self.hyperion_cli_args_ptr;

        let argc: usize = Self::pop(&mut ptr);
        let mut arg_lengths = ptr;
        let mut arg_strings = ptr + argc * core::mem::size_of::<usize>();

        (0..argc).map(move |_| {
            let len: usize = Self::pop(&mut arg_lengths);
            let str: &[u8] = unsafe { core::slice::from_raw_parts(arg_strings as _, len as _) };
            arg_strings += len;

            unsafe { core::str::from_utf8_unchecked(str) }
        })
    }

    fn pop<T: Sized>(top: &mut usize) -> T {
        let v = unsafe { ((*top) as *const T).read() };
        *top += core::mem::size_of::<T>();
        v
    }
}

impl fmt::Debug for CliArgs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

pub struct PageAlloc;

unsafe impl GlobalAlloc for PageAlloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let pages = layout.size().div_ceil(0x1000);

        let res = palloc(pages);
        // println!("alloc syscall res: {res:?}");
        res.expect("page alloc").expect("null alloc").as_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let pages = layout.size().div_ceil(0x1000);
        assert!(pfree(NonNull::new(ptr).unwrap(), pages).is_ok());
    }
}

#[global_allocator]
static GLOBAL_ALLOC: PageAlloc = PageAlloc;
