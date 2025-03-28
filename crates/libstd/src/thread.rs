use core::{
    arch::naked_asm,
    mem::{self, MaybeUninit},
    ptr::{self, DynMetadata, NonNull},
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use hyperion_syscall::{done, fs::FileDesc, InvalidSyscall, MemMapFlags};

use crate::{
    rt::{MAIN_STACK_GUARD_BOTTOM, MAIN_STACK_SIZE, STACK_GUARD_SIZE},
    sync::Mutex,
};

//

pub fn spawn<F: FnOnce() + Send + 'static>(f: F) {
    let f_inner = MaybeUninit::new(f);
    let f = move || {
        unsafe { f_inner.assume_init_read()() };
    };

    // allocate a new stack for the new thread
    let mut sp = alloc_stack() as usize;

    let meta = ptr::metadata(&f as &(dyn Fn() + Send + 'static));

    fn push<T>(sp: &mut usize, val: T) {
        *sp -= mem::size_of::<T>();
        *sp &= !(mem::align_of::<T>() - 1);
        unsafe { (*sp as *mut T).write_volatile(val) };
    }

    push(&mut sp, f);
    let data_ptr = sp;

    push(&mut sp, meta);
    let meta_ptr = sp;

    hyperion_syscall::log!("meta_ptr={meta_ptr:?} data_ptr={data_ptr:}");

    push(&mut sp, data_ptr);
    push(&mut sp, meta_ptr);

    // spawn a new process in the same memory space with
    // `sp` as its stack, running `_thread_entry`
    hyperion_syscall::spawn(_thread_entry, sp);
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

    stack_ptr as _
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

#[no_mangle]
#[naked]
extern "C" fn _thread_entry() -> ! {
    unsafe {
        naked_asm!("mov rdi, rsp", "jmp _thread_entry_rust");
    }
}

#[no_mangle]
extern "C" fn _thread_entry_rust(sp: usize) -> ! {
    hyperion_syscall::log!("_thread_entry_rust");
    let meta_ptr = sp as *mut DynMetadata<dyn Fn() + Send + 'static>;
    let data_ptr = (sp + mem::size_of::<usize>()) as *mut ();

    hyperion_syscall::log!("meta_ptr={meta_ptr:?} data_ptr={data_ptr:?}");

    let metadata = unsafe { meta_ptr.read_volatile() };

    hyperion_syscall::log!("exec entry fn");
    let entry_fn = ptr::from_raw_parts_mut::<dyn Fn() + Send + 'static>(data_ptr, metadata);

    unsafe { (*entry_fn)() };

    done(0);
}
