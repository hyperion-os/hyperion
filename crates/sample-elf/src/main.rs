#![no_std]
#![no_main]
#![feature(format_args_nl, slice_internals)]

//

extern crate alloc;

use alloc::{boxed::Box, string::String, sync::Arc};
use core::{
    alloc::GlobalAlloc,
    fmt::{self, Write},
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperion_syscall::*;

use crate::io::{BufReader, SimpleIpcInputChannel};

//

mod io; // partial std::io

//

pub fn main(args: CliArgs) {
    println!("sample app main");
    println!("args: {args:?}");

    match args.iter().next().expect("arg0 to be present") {
        // busybox style single binary 'coreutils'
        "/bin/run" => {
            let inc = Arc::new(AtomicUsize::new(0));

            for _n in 0..80 {
                let inc = inc.clone();
                spawn(move || {
                    inc.fetch_add(1, Ordering::Relaxed);
                    // println!("print from thread {n}");
                });
            }

            let mut next = timestamp().unwrap() as u64;
            for i in next / 1_000_000_000.. {
                println!("inc at: {}", inc.load(Ordering::Relaxed));

                nanosleep_until(next);
                next += 1_000_000_000;

                println!("seconds since boot: {i}");
            }
        }

        "/bin/task1" => {
            rename("<Get_Input>").unwrap();

            let pid: u64 = args
                .iter()
                .nth(1)
                .expect("missing arg: PID")
                .parse()
                .expect("failed to parse PID");

            let mut line = String::new();
            loop {
                line.clear();
                let mut input_channel = BufReader::new(SimpleIpcInputChannel);
                input_channel.read_line(&mut line).unwrap();

                let input = line.trim();
                println!("<Get_Input>: '{input}'");
                send(pid, input.as_bytes()).unwrap();
                send(pid, b"\n").unwrap(); // BufReader::read_line waits for a \n
            }
        }

        "/bin/task2" => {
            rename("<Clean_Input>").unwrap();

            let pid: u64 = args
                .iter()
                .nth(1)
                .expect("missing arg: PID")
                .parse()
                .expect("failed to parse PID");

            let mut line = String::new();
            loop {
                line.clear();
                let mut input_channel = BufReader::new(SimpleIpcInputChannel);
                input_channel.read_line(&mut line).unwrap();

                let messy_string = line.trim();
                let clean_string = messy_string.replace(|c| !char::is_alphabetic(c), "");
                println!("<Clean_Input>: '{clean_string}'");

                send(pid, clean_string.as_bytes()).unwrap();
                send(pid, b"\n").unwrap(); // BufReader::read_line waits for a \n
            }
        }

        "/bin/task3" => {
            rename("<Find_Missing>").unwrap();

            let mut line = String::new();

            loop {
                line.clear();
                let mut input_channel = BufReader::new(SimpleIpcInputChannel);
                input_channel.read_line(&mut line).unwrap();

                let mut found = [false; 26];
                for c in line.trim().chars() {
                    found[((c as u8).to_ascii_lowercase() - b'a') as usize] = true;
                }

                let mut buf = String::new();
                for missing in found
                    .iter()
                    .enumerate()
                    .filter(|(_, found)| !*found)
                    .map(|(i, _)| i)
                {
                    buf.push((missing as u8 + b'a') as char);
                }
                println!("<Find_Missing>: '{buf}'");

                // PID 1 is known to be kshell, for now
                // send(1, buf.as_bytes());
            }
        }

        tool => panic!("unknown tool {tool}"),
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
        palloc(pages as u64).expect("page alloc") as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let pages = layout.size().div_ceil(0x1000);
        assert!(pfree(ptr as u64, pages as u64).is_ok());
    }
}

#[global_allocator]
static GLOBAL_ALLOC: PageAlloc = PageAlloc;

//

#[allow(unused)]
fn spawn(f: impl FnOnce() + Send + 'static) {
    let f_fatptr: Box<dyn FnOnce() + Send + 'static> = Box::new(f);
    let f_fatptr_box: *mut Box<dyn FnOnce() + Send + 'static> = Box::into_raw(Box::new(f_fatptr));

    pthread_spawn(_thread_entry, f_fatptr_box as _);
}

fn _print(args: fmt::Arguments) {
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
extern "C" fn _start(a0: u64) -> ! {
    main(CliArgs {
        hyperion_cli_args_ptr: a0 as _,
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
