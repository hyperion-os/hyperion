#![no_std]

//

use alloc::{boxed::Box, sync::Arc};
use core::{
    any::Any,
    cell::UnsafeCell,
    mem::swap,
    sync::atomic::{AtomicUsize, Ordering},
};

use crossbeam_queue::SegQueue;
use hyperion_arch::{
    context::Context,
    cpu::ints::{self, PageFaultResult, Privilege},
    stack::{AddressSpace, KernelStack, Stack},
    tls,
    vmm::PageMap,
};
use hyperion_mem::vmm::PageMapImpl;
use hyperion_scheduler_task::{AnyTask, CleanupTask, Task};
use x86_64::VirtAddr;

//

extern crate alloc;

pub mod executor;
pub mod keyboard;
pub mod process;
pub mod task;
pub mod timer;

//

pub struct TaskImpl {
    address_space: Arc<AddressSpace>,
    stack: Stack<KernelStack>,

    // context is used 'unsafely' only in the switch
    context: UnsafeCell<Context>,
    job: Option<Box<dyn FnOnce() + Send + 'static>>,
    pid: usize,
}

impl TaskImpl {
    pub fn new(f: impl FnOnce() + Send + 'static) -> Self {
        hyperion_log::trace!("new task");
        static NEXT_PID: AtomicUsize = AtomicUsize::new(0);
        let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);

        hyperion_log::trace!("new address space");
        let address_space = Arc::new(AddressSpace::new(PageMap::new()));

        let mut stack = address_space.kernel_stacks.take();
        hyperion_log::trace!("initializing task stack");
        // stack.init(&address_space.page_map);
        stack.grow(&address_space.page_map).unwrap();
        stack.grow(&address_space.page_map).unwrap();
        stack.grow(&address_space.page_map).unwrap();
        stack.grow(&address_space.page_map).unwrap();

        let context = UnsafeCell::new(Context::new(
            &address_space.page_map,
            stack.top,
            stack.base_alloc + 0x1000u64,
            thread_entry,
        ));
        let job = Some(Box::new(f) as _);

        Self {
            address_space,
            context,
            stack,
            job,
            pid,
        }
    }

    pub fn debug(&mut self) {
        hyperion_log::debug!(
            "TASK DEBUG: context: {:0x?}, job: {:?}, pid: {}",
            unsafe { &*self.context.get() },
            self.job.as_ref().map(|_| ()),
            self.pid
        )
    }
}

impl AnyTask for TaskImpl {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn take_job(&mut self) -> Option<Box<dyn FnOnce() + Send + 'static>> {
        self.job.take()
    }

    fn pid(&self) -> usize {
        self.pid
    }
}

pub static READY: SegQueue<Task> = SegQueue::new();

/// reset this processors scheduling
pub fn reset() -> ! {
    ints::PAGE_FAULT_HANDLER.store(page_fault_handler);

    let boot: Task = Box::new(TaskImpl::new(|| {}));
    *tls::get().active.lock() = Some(boot);
    stop();
}

/// switch to another thread
pub fn yield_now() {
    let Some(mut current) = swap_current(None) else {
        return;
        // unreachable!("cannot yield from a task that doesn't exist")
    };

    let Some(task): Option<&mut TaskImpl> = current.as_any().downcast_mut() else {
        unreachable!("the task was from another scheduler")
    };
    let context = task.context.get();

    // push the current thread back to the ready queue AFTER switching
    // task.debug();
    tls::get().after_switch.push(CleanupTask::Ready(current));

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
    let Some(mut current) = swap_current(None) else {
        unreachable!("cannot stop a task that doesn't exist")
    };

    let Some(context): Option<&mut TaskImpl> = current.as_any().downcast_mut() else {
        unreachable!("the task was from another scheduler")
    };
    let context = context.context.get();

    // push the current thread to the drop queue AFTER switching
    // current.debug();
    tls::get().after_switch.push(CleanupTask::Drop(current));

    // SAFETY: `current` is stored in the queue until the switch
    // and the boxed field `context` makes sure the context pointer doesn't move
    unsafe {
        block(context);
    }

    unreachable!("a destroyed thread cannot continue executing");
}

pub fn spawn(f: impl FnOnce() + Send + 'static) {
    schedule(Box::new(TaskImpl::new(f)))
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
    hyperion_log::debug!("stopping thread1");
    let mut next = next_task();
    hyperion_log::debug!("stopping thread2");

    let Some(task): Option<&mut TaskImpl> = next.as_any().downcast_mut() else {
        unreachable!("the task was from another scheduler")
    };
    hyperion_log::debug!("stopping thread3");
    let context = task.context.get();
    hyperion_log::debug!("stopping thread4");

    let rsp: u64;
    unsafe {
        core::arch::asm!("mov {rsp}, rsp", rsp = lateout(reg) rsp);
    }
    hyperion_log::debug!("DBG stop rsp (rsp:{rsp:0x})");

    hyperion_log::debug!("stopping thread tls = {:0x}", tls::get() as *const _ as u64);
    /* unsafe {
        core::arch::asm!("2:", "jmp 2b");
    } */
    // task.debug();
    tls::get().after_switch.push(CleanupTask::Next(next));
    hyperion_log::debug!("stopping thread5");

    // SAFETY: `next` is stored in the queue until the switch
    // and the boxed field `context` makes sure the context pointer doesn't move
    unsafe {
        hyperion_log::debug!("{:?}", &*context);
        hyperion_arch::context::switch(current, context);
    }

    cleanup();
}

pub fn next_task() -> Task {
    // loop {
    for _ in 0..1000 {
        if let Some(next) = READY.pop() {
            return next;
        }

        // TODO: halt until the next task arrives
    }

    // give up and run a none task
    hyperion_log::debug!("no jobs");
    Box::new(TaskImpl::new(|| {}))
}

pub fn cleanup() {
    loop {
        match tls::get().after_switch.pop() {
            Some(CleanupTask::Ready(ready)) => {
                schedule(ready);
            }
            Some(CleanupTask::Next(next)) => {
                swap_current(Some(next));
            }
            Some(CleanupTask::Drop(_drop)) => {}
            None => break,
        };
    }
}

fn page_fault_handler(addr: usize, user: Privilege) -> PageFaultResult {
    hyperion_log::debug!("PAGE FAULT ({user:?}) 0x{addr:0x}");

    let Some(mut current) = swap_current(None) else {
        hyperion_log::debug!("no job");
        return PageFaultResult::NotHandled;
    };

    if user == Privilege::User {
        swap_current(Some(current));
        stop();
    }

    let Some(task): Option<&mut TaskImpl> = current.as_any().downcast_mut() else {
        hyperion_log::debug!("no task");
        return PageFaultResult::NotHandled;
    };

    let result = task
        .stack
        .page_fault(&task.address_space.page_map, addr as u64);

    hyperion_log::debug!("PAGE FAULT HANDLER {result:?}");
    swap_current(Some(current));

    result
}

extern "sysv64" fn thread_entry() -> ! {
    let rsp: u64;
    unsafe {
        core::arch::asm!("mov {rsp}, rsp", rsp = lateout(reg) rsp);
    }
    hyperion_log::debug!("thread_entry rsp (rsp:{rsp:0x})");

    hyperion_log::debug!("thread_entry");
    cleanup();
    stop();
    hyperion_log::debug!("1");
    {
        hyperion_log::debug!("2");
        let Some(mut current) = swap_current(None) else {
            unreachable!("cannot run a task that doesn't exist")
        };
        hyperion_log::debug!("2");
        let Some(job) = current.take_job() else {
            unreachable!("cannot run a task that already ran")
        };
        hyperion_log::debug!("3");
        swap_current(Some(current));
        hyperion_log::debug!("4");
        // job();

        /* #[allow(unconditional_recursion)]
        fn stack_overflow() {
            core::hint::black_box(stack_overflow)();
        }

        stack_overflow(); */

        // hyperion_log::debug!("5: {:?}", core::hint::black_box([40u64; 64]));
    }

    stop();
}
