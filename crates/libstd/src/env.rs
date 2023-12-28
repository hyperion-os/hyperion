use core::{fmt, slice};

use core_alloc::{string::String, vec::Vec};

//

#[must_use]
pub fn args() -> Args {
    Args {
        top: unsafe { ARGS }.iter(),
    }
}

pub(crate) unsafe fn init_args(hyperion_cli_args_ptr: usize) {
    let stack_args = CliArgs {
        hyperion_cli_args_ptr,
    };

    let args = stack_args
        .iter()
        .map(|arg| &*String::from(arg).leak())
        .collect::<Vec<_>>()
        .leak();

    unsafe { ARGS = args };
}

//

#[derive(Clone)]
pub struct Args {
    top: slice::Iter<'static, &'static str>,
}

impl fmt::Debug for CliArgs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl Iterator for Args {
    type Item = &'static str;

    fn next(&mut self) -> Option<Self::Item> {
        self.top.next().copied()
    }
}

impl DoubleEndedIterator for Args {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.top.next_back().copied()
    }
}

impl ExactSizeIterator for Args {
    fn len(&self) -> usize {
        self.top.len()
    }

    // fn is_empty(&self) -> bool {
    //     self.top.is_empty()
    // }
}

//

static mut ARGS: &[&str] = &[];

#[derive(Clone, Copy)]
struct CliArgs {
    hyperion_cli_args_ptr: usize,
}

impl CliArgs {
    fn iter(self) -> impl DoubleEndedIterator<Item = &'static str> + Clone {
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
