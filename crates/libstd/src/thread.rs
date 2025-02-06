use core::{
    mem,
    ptr::{self, NonNull},
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use hyperion_syscall::{done, fs::FileDesc, MemMapFlags};

use crate::{
    rt::{MAIN_STACK_GUARD_BOTTOM, MAIN_STACK_SIZE, STACK_GUARD_SIZE},
    sync::Mutex,
};

//

pub fn spawn<F: FnOnce() + Send + 'static>(f: F) {
    // allocate a new stack for the new thread
    let mut sp = alloc_stack() as usize;

    // allocate memory from the stack for `f`
    sp -= mem::size_of::<F>();
    // align it correctly
    sp &= !(mem::align_of::<F>() - 1);
    // write `f` into the stack
    unsafe { (sp as *mut F).write_volatile(f) };

    // spawn a new process in the same memory space with
    // `sp` as its stack, running `thread_entry`
    hyperion_syscall::spawn(thread_entry, sp);
}

/// returns a pointer to the top of a new stack
pub fn alloc_stack() -> *mut () {
    let mut stacks = STACK_CHAIN.lock();
    let next = stacks.load(Ordering::Relaxed);
    if let Some(next) = NonNull::new(next) {
        // got a cached stack
        *stacks = unsafe { next.read() }.next;
        let top = next.as_ptr() as usize + MAIN_STACK_SIZE;
        return top as _;
    }
    drop(stacks);

    // allocating a new stack
    let stack_ptr = NEXT_STACK_TOP.fetch_sub(MAIN_STACK_SIZE + STACK_GUARD_SIZE, Ordering::Relaxed);
    if stack_ptr <= 0x1000 + MAIN_STACK_SIZE + STACK_GUARD_SIZE {
        panic!("too many active stacks");
    }
    let guard_bottom = stack_ptr - STACK_GUARD_SIZE - MAIN_STACK_SIZE;

    // the kernel might have moved it
    let guard_bottom = hyperion_syscall::mem_map(
        NonNull::new(guard_bottom as _),
        MAIN_STACK_SIZE + STACK_GUARD_SIZE,
        MemMapFlags::RW | MemMapFlags::ANON,
        FileDesc(0),
        0,
    )
    .unwrap();
    hyperion_syscall::mem_map(
        Some(guard_bottom),
        STACK_GUARD_SIZE,
        MemMapFlags::FIXED | MemMapFlags::ANON, // no read, no write, no exec => guard page
        FileDesc(0),
        0,
    )
    .unwrap();

    let top = MAIN_STACK_SIZE + STACK_GUARD_SIZE;
    top as _
}

/// # Safety
/// must've been allocated with [`alloc_stack`]
pub unsafe fn free_stack(top: *mut ()) {
    let stack_bottom = top as usize - STACK_GUARD_SIZE;

    let mut stacks = STACK_CHAIN.lock();
    unsafe {
        (stack_bottom as *mut StackChain).write(StackChain {
            next: AtomicPtr::new(*stacks.get_mut()),
        });
    }
    *stacks = AtomicPtr::new(stack_bottom as _);
}

//

static NEXT_STACK_TOP: AtomicUsize = AtomicUsize::new(MAIN_STACK_GUARD_BOTTOM);
static STACK_CHAIN: Mutex<AtomicPtr<StackChain>> = Mutex::new(AtomicPtr::new(ptr::null_mut()));

//

struct StackChain {
    next: AtomicPtr<StackChain>,
}

//

extern "C" fn thread_entry(ip: usize, sp: usize) -> ! {
    crate::println!("_thread_entry(ip={ip:#x}, sp={sp:#x})");
    // println!("_thread_entry {_stack_ptr} {arg}");
    // let f_fatptr_box: *mut Box<dyn FnOnce() + Send + 'static> = arg as _;
    // let f_fatptr: Box<dyn FnOnce() + Send + 'static> = *unsafe { Box::from_raw(f_fatptr_box) };

    // println!("addr {:0x}", (&*f_fatptr) as *const _ as *const () as usize);

    // f_fatptr();
    // println!("_thread_entry f call");

    // TODO: pthread_exit + exit should kill all threads
    done(0);
}
