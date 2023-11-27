use alloc::boxed::Box;

use hyperion_syscall::{exit, pthread_spawn};

//

pub fn spawn(f: impl FnOnce() + Send + 'static) {
    let f_fatptr: Box<dyn FnOnce() + Send + 'static> = Box::new(f);
    let f_fatptr_box: *mut Box<dyn FnOnce() + Send + 'static> = Box::into_raw(Box::new(f_fatptr));

    pthread_spawn(_thread_entry, f_fatptr_box as _);
}

extern "C" fn _thread_entry(_stack_ptr: usize, arg: usize) -> ! {
    // println!("_thread_entry");
    // println!("_thread_entry {_stack_ptr} {arg}");
    let f_fatptr_box: *mut Box<dyn FnOnce() + Send + 'static> = arg as _;
    let f_fatptr: Box<dyn FnOnce() + Send + 'static> = *unsafe { Box::from_raw(f_fatptr_box) };

    // println!("addr {:0x}", (&*f_fatptr) as *const _ as *const () as usize);

    f_fatptr();
    // println!("_thread_entry f call");

    // TODO: pthread_exit + exit should kill all threads
    exit(0);
}
