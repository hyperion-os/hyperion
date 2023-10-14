#![no_std]
#![no_main]
#![feature(format_args_nl)]

//

extern crate alloc;

use alloc::boxed::Box;
use core::{
    alloc::GlobalAlloc,
    fmt::{self, Write},
};

use hyperion_syscall::*;

//

pub fn main(args: CliArgs) {
    println!("sample app main");
    println!("args: {args:?}");

    spawn(|| {
        println!("print from thread 2");
    });

    let mut next = timestamp().unwrap() as u64;
    for i in 0.. {
        nanosleep_until(next);
        next += 1_000_000_000;

        println!("seconds since boot: {i}");
    }
}

//

#[macro_export]
macro_rules! println {
    ($($v:tt)*) => {
        _print(format_args_nl!($($v)*))
    };
}

//

#[derive(Clone, Copy)]
pub struct CliArgs {
    hyperion_cli_args_ptr: u64,
}

impl CliArgs {
    pub fn iter(self) -> impl Iterator<Item = &'static str> + Clone + DoubleEndedIterator {
        let mut ptr = self.hyperion_cli_args_ptr;

        let argc: u64 = Self::pop(&mut ptr);
        let mut arg_lengths = ptr;
        let mut arg_strings = ptr + argc * core::mem::size_of::<u64>() as u64;

        (0..argc).map(move |_| {
            let len: u64 = Self::pop(&mut arg_lengths);
            let str: &[u8] = unsafe { core::slice::from_raw_parts(arg_strings as _, len as _) };
            arg_strings += len;

            unsafe { core::str::from_utf8_unchecked(str) }
        })
    }

    fn pop<T: Sized>(top: &mut u64) -> T {
        let v = unsafe { ((*top) as *const T).read() };
        *top += core::mem::size_of::<T>() as u64;
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
        let alloc = palloc(pages as u64);

        if alloc <= 0 {
            panic!("page alloc failed: {alloc}");
        }

        alloc as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let pages = layout.size().div_ceil(0x1000);
        pfree(ptr as u64, pages as u64);
    }
}

#[global_allocator]
static GLOBAL_ALLOC: PageAlloc = PageAlloc;

//

fn spawn(f: impl FnOnce() + Send + 'static) {
    let f_fatptr: Box<dyn FnOnce() + Send + 'static> = Box::new(f);
    let f_fatptr_box: *mut Box<dyn FnOnce() + Send + 'static> = Box::into_raw(Box::new(f_fatptr));

    pthread_spawn(_thread_entry, f_fatptr_box as u64);
}

fn _print(args: fmt::Arguments) {
    struct SyscallLog;

    //

    impl Write for SyscallLog {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            if hyperion_syscall::log(s) == 0 {
                Ok(())
            } else {
                Err(fmt::Error)
            }
        }
    }

    _ = SyscallLog.write_fmt(args);
}

//

#[no_mangle]
extern "C" fn _start(a0: u64) -> ! {
    main(CliArgs {
        hyperion_cli_args_ptr: a0,
    });
    exit(0);
}

extern "C" fn _thread_entry(_stack_ptr: u64, arg: u64) -> ! {
    let f_fatptr_box: *mut Box<dyn FnOnce() + Send + 'static> = arg as _;
    let f_fatptr: Box<dyn FnOnce() + Send + 'static> = *unsafe { Box::from_raw(f_fatptr_box) };

    f_fatptr();

    exit(0);
}

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    println!("{info}");
    exit(-1);
}
