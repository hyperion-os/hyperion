#![no_std]
#![no_main]
#![feature(format_args_nl, slice_internals)]

//

extern crate alloc;

use alloc::{boxed::Box, string::String, sync::Arc};
use core::{
    alloc::GlobalAlloc,
    fmt::{self, Write},
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperion_syscall::{fs::File, *};

use crate::io::{BufReader, SimpleIpcInputChannel};

//

mod io; // partial std::io

//

pub fn main(args: CliArgs) {
    println!("sample app main");
    println!("args: {args:?}");

    // for i in 1..13 {
    //     if i == 2 || i == 8 {
    //         continue;
    //     }
    //     println!("{:?}", unsafe { syscall_0(i) });
    // }

    match args.iter().next().expect("arg0 to be present") {
        // busybox style single binary 'coreutils'
        "/bin/run" => {
            let inc = Arc::new(AtomicUsize::new(0));

            for _n in 0..80 {
                let inc = inc.clone();
                spawn(move || {
                    // println!("hello from thread {_n}");
                    inc.fetch_add(1, Ordering::Relaxed);
                });
            }

            let hpet = File::open("/dev/hpet").expect("failed to open /dev/hpet");
            let mut buf = [0u8; 256];
            let bytes = hpet.read(&mut buf).expect("failed to read from a file");

            println!("/dev/hpet bytes: {:?}", &buf[..bytes]);
            drop(hpet);

            let file = File::open("/testfile").expect("failed to open /testfile");
            file.write(b"testing data").expect("failed to write");
            drop(file);

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

                println!("got '{}'", line.trim());

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
extern "C" fn _start(a0: usize) -> ! {
    main(CliArgs {
        hyperion_cli_args_ptr: a0,
    });
    exit(0);
}

extern "C" fn _thread_entry(_stack_ptr: usize, arg: usize) -> ! {
    // println!("_thread_entry");
    // println!("_thread_entry {_stack_ptr} {arg}");
    let f_fatptr_box: *mut Box<dyn FnOnce() + Send + 'static> = arg as _;
    let f_fatptr: Box<dyn FnOnce() + Send + 'static> = *unsafe { Box::from_raw(f_fatptr_box) };

    // println!("addr {:0x}", (&*f_fatptr) as *const _ as *const () as usize);

    f_fatptr();
    // println!("_thread_entry f call");

    exit(0);
}

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    println!("sample-elf: {info}");
    exit(-1);
}
