use alloc::boxed::Box;
use core::{
    cell::UnsafeCell,
    mem::swap,
    sync::atomic::{AtomicUsize, Ordering},
};

use crossbeam::queue::SegQueue;
use hyperion_mem::pmm::PageFrameAllocator;
use memoffset::offset_of;
use x86_64::{registers::control::Cr3, PhysAddr, VirtAddr};

use crate::tls;

//

#[derive(Debug)]
#[repr(C)]
pub struct Context {
    pub rsp: VirtAddr,
    pub cr3: PhysAddr,
    pub pid: usize,
}

impl Context {
    pub fn new(pid: usize) -> Self {
        let mut stack = PageFrameAllocator::get().alloc(10);

        // hyperion_log::trace!(
        //     "task: {:0x}..{:x}",
        //     stack.virtual_addr(),
        //     stack.virtual_addr() + 0x1000u64
        // );

        let stack_slice: &mut [u64] = stack.as_mut_slice();
        let [top @ .., _r15, _r14, _r13, _r12, _rbx, _rbp, entry] = stack_slice else {
            unreachable!("the stack is too small")
        };

        *entry = thread_entry as *const () as u64;

        Self {
            cr3: Cr3::read().0.start_address(),
            rsp: VirtAddr::new(top.as_ptr_range().end as u64),
            pid,
        }
    }
}

/* impl Drop for Context {
    fn drop(&mut self) {
        hyperion_log::trace!("dropping context (PID:{})", self.pid);
    }
} */

pub struct Task {
    // context is used 'unsafely' only in the switch
    context: Box<UnsafeCell<Context>>,
    job: Option<Box<dyn FnOnce() + Send + 'static>>,
    pid: usize,
}

impl Task {
    pub fn new(f: impl FnOnce() + Send + 'static) -> Self {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(0);

        let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);

        Self {
            context: Box::new(UnsafeCell::new(Context::new(pid))),
            job: Some(Box::new(f)),
            pid,
        }
    }

    pub fn debug(&mut self) {
        hyperion_log::debug!(
            "TASK DEBUG: context: {:0x}, job: {:?}, pid: {}",
            unsafe { (*self.context.get()).rsp },
            self.job.as_ref().map(|_| ()),
            self.pid
        )
    }
}

pub static READY: SegQueue<Task> = SegQueue::new();

/// reset this processors scheduling
pub fn reset() -> ! {
    let boot = Task::new(|| {});
    *tls::get().active.lock() = Some(boot);
    stop();
}

#[inline(always)]
pub fn ip() -> u64 {
    x86_64::instructions::read_rip().as_u64()
}

/// switch to another thread
pub fn yield_now() {
    let Some(current) = swap_current(None) else {
        unreachable!("cannot yield from a task that doesn't exist")
    };

    // push the current thread back to the ready queue AFTER switching
    // current.debug();
    let context = current.context.get();
    tls::get().free_thread.push(current);

    // SAFETY: `current` is stored in the queue until the switch
    // and the boxed field `context` makes sure the context pointer doesn't move
    unsafe {
        block(context);
    }
}

/// destroy the current thread
/// and switch to another thread
pub fn stop() -> ! {
    // hyperion_log::debug!("stop");
    let Some(current) = swap_current(None) else {
        unreachable!("cannot stop a task that doesn't exist")
    };

    // push the current thread to the drop queue AFTER switching
    // current.debug();
    let context = current.context.get();
    tls::get().drop_thread.push(current);

    // SAFETY: `current` is stored in the queue until the switch
    // and the boxed field `context` makes sure the context pointer doesn't move
    unsafe {
        block(context);
    }

    unreachable!("a destroyed thread cannot continue executing");
}

/// schedule
pub fn schedule(new: Task) {
    READY.push(new);
}

pub fn swap_current(mut new: Option<Task>) -> Option<Task> {
    swap(&mut new, &mut tls::get().active.lock());
    new
}

/// # Safety
///
/// `current` must be correct and point to a valid exclusive [`Context`]
pub unsafe fn block(current: *mut Context) {
    let next = next_task();

    // next.debug();
    let context = next.context.get();
    tls::get().next_thread.push(next);

    // SAFETY: `next` is stored in the queue until the switch
    // and the boxed field `context` makes sure the context pointer doesn't move
    unsafe {
        switch(current, context);
    }

    cleanup();
}

pub fn next_task() -> Task {
    // loop {
    for _ in 0..1000 {
        if let Some(next) = READY.pop() {
            return next;
        }

        // hyperion_log::debug!("no jobs");

        // TODO: halt until the next task arrives
    }

    // give up and run a none task
    Task::new(|| {})
}

pub fn cleanup() {
    while let Some(free) = tls::get().free_thread.pop() {
        READY.push(free);
    }
    while let Some(_next) = tls::get().drop_thread.pop() {}
    while let Some(next) = tls::get().next_thread.pop() {
        swap_current(Some(next));
    }
}

extern "sysv64" fn thread_entry() -> ! {
    cleanup();
    {
        let Some(mut current) = swap_current(None) else {
            unreachable!("cannot run a task that doesn't exist")
        };
        let Some(job) = current.job.take() else {
            unreachable!("cannot run a task that already ran")
        };
        swap_current(Some(current));
        job();
    }
    stop();
}

//

/// # Safety
///
/// both `prev` and `next` must be correct and point to valid exclusive [`Context`] values
/// even after switching the new address spacing according to the field `cr3` in `next`
#[naked]
pub unsafe extern "sysv64" fn switch(prev: *mut Context, next: *mut Context) {
    // TODO: fx(save/rstor)64 (rd/wr)(fs/gs)base

    core::arch::asm!(
        // save callee-saved registers
        // https://wiki.osdev.org/System_V_ABI
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // save prev task
        "mov [rdi+{rsp}], rsp", // save prev stack

        // load next task
        "mov rsp, [rsi+{rsp}]", // load next stack
        "mov rax, [rsi+{cr3}]", // rax = next virtual address space
        // TODO: load TSS privilege stack

        // optional virtual address space switch
        "mov rcx, cr3", // rcx = prev virtual address space
        "cmp rax, rcx", // cmp for if
        "je 2f",         // if rax != rcx:
        "mov cr3, rax", // load next virtual address space

        "2:",

        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",

        "ret",

        rsp = const(offset_of!(Context, rsp)),
        cr3 = const(offset_of!(Context, cr3)),
        options(noreturn)
    );
}
