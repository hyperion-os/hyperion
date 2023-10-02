#![no_std]
#![feature(new_uninit, type_name_of_val, extract_if)]

//

use alloc::{
    boxed::Box,
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};
use core::{
    any::type_name_of_val,
    cell::UnsafeCell,
    mem::swap,
    ops::DerefMut,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use crossbeam::atomic::AtomicCell;
use crossbeam_queue::SegQueue;
use hyperion_arch::{
    context::Context,
    cpu::ints,
    stack::{AddressSpace, KernelStack, Stack, UserStack},
    tls::Tls,
    vmm::PageMap,
};
use hyperion_driver_acpi::hpet::HPET;
use hyperion_log::*;
use hyperion_mem::vmm::{PageFaultResult, PageMapImpl, Privilege};
use hyperion_random::Rng;
use spin::{Lazy, Mutex};

//

extern crate alloc;

pub mod executor;
pub mod keyboard;
pub mod process;
pub mod task;
pub mod timer;

//

static MAGIC_DEBUG_BYTE: Lazy<usize> = Lazy::new(|| hyperion_random::next_fast_rng().gen());

//

pub enum CleanupTask {
    Next(Task),
    Drop(Task),
    Ready(Task),
}

pub struct Task {
    memory: Arc<TaskMemory>,

    // context is used 'unsafely' only in the switch
    context: Box<UnsafeCell<Context>>,
    job: Option<Box<dyn FnOnce() + Send + 'static>>,

    info: Arc<TaskInfo>,
}

pub struct TaskMemory {
    address_space: AddressSpace,
    kernel_stack: Mutex<Stack<KernelStack>>,
    user_stack: Mutex<Stack<UserStack>>,
    dbg_magic_byte: usize,
}

#[derive(Debug)]
pub struct TaskInfo {
    // proc id
    pub pid: usize,

    // proc name
    pub name: &'static str,

    // cpu time used
    pub nanos: AtomicU64,

    // proc state
    pub state: AtomicCell<TaskState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Running,
    Waiting,
    Ready,
    Dropping,
}

impl Task {
    pub fn new(f: impl FnOnce() + Send + 'static) -> Self {
        let name = type_name_of_val(&f);
        let job = Some(Box::new(f) as _);
        debug!("initializing task {name}");

        let info = Arc::new(TaskInfo {
            pid: Self::next_pid(),
            name,
            nanos: AtomicU64::new(0),
            state: AtomicCell::new(TaskState::Ready),
        });
        TASKS.lock().push(Arc::downgrade(&info));

        let address_space = AddressSpace::new(PageMap::new());

        let mut kernel_stack = address_space.kernel_stacks.take();
        kernel_stack.grow(&address_space.page_map, 2).unwrap();
        // kernel_stack.grow(&address_space.page_map, 6).unwrap();
        let stack_top = kernel_stack.top;
        let user_stack = address_space.user_stacks.take();

        let memory = Arc::new(TaskMemory {
            address_space,
            kernel_stack: Mutex::new(kernel_stack),
            user_stack: Mutex::new(user_stack),
            dbg_magic_byte: *MAGIC_DEBUG_BYTE,
        });

        /* let alloc = PFA.alloc(1);
        memory.address_space.page_map.map(
            CURRENT_ADDRESS_SPACE..CURRENT_ADDRESS_SPACE + 0xFFFu64,
            alloc.physical_addr(),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        ); */
        /* let current_address_space: *mut Arc<TaskMemory> =
            to_higher_half(alloc.physical_addr()).as_mut_ptr();
        unsafe { current_address_space.write(memory.clone()) }; */

        let context = Box::new(UnsafeCell::new(Context::new(
            &memory.address_space.page_map,
            stack_top,
            thread_entry,
        )));

        Self {
            memory,

            context,
            job,
            info,
        }
    }

    pub fn next_pid() -> usize {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(0);
        NEXT_PID.fetch_add(1, Ordering::Relaxed)
    }

    pub fn info(&self) -> &TaskInfo {
        &self.info
    }

    pub fn debug(&mut self) {
        hyperion_log::debug!(
            "TASK DEBUG: context: {:0x?}, job: {:?}, pid: {}",
            &self.context as *const _ as usize,
            // unsafe { &*self.context.get() },
            self.job.as_ref().map(|_| ()),
            self.info.pid
        )
    }
}

pub static READY: SegQueue<Task> = SegQueue::new();
pub static TASKS: Mutex<Vec<Weak<TaskInfo>>> = Mutex::new(vec![]);

pub fn tasks() -> Vec<Arc<TaskInfo>> {
    let mut tasks = TASKS.lock();

    // remove tasks that don't exist
    tasks.retain(|p| p.strong_count() != 0);

    tasks.iter().filter_map(|p| p.upgrade()).collect()
}

/// reset this processors scheduling
pub fn reset() -> ! {
    hyperion_arch::int::disable();

    ints::PAGE_FAULT_HANDLER.store(page_fault_handler);
    // TODO: hyperion_driver_acpi::apic::APIC_TIMER_HANDLER.store(|| yield_now());

    swap_current(Some(Task::new(|| {})));
    stop();
}

/// switch to another thread
pub fn yield_now() {
    let Some(next) = next_task() else {
        // no other tasks, don't switch
        return;
    };
    let Some(current) = swap_current(None) else {
        unreachable!("cannot yield from a task that doesn't exist")
    };
    if current.info.state.load() == TaskState::Running {
        current.info.state.store(TaskState::Ready);
    };
    update_cpu_usage(&current);

    let context = current.context.get();

    // push the current thread back to the ready queue AFTER switching
    after().push(CleanupTask::Ready(current));

    // SAFETY: `current` is stored in the queue until the switch
    // and the boxed field `context` makes sure the context pointer doesn't move
    unsafe {
        block(context, next);
    }
}

/// destroy the current thread
/// and switch to another thread
pub fn stop() -> ! {
    // TODO: running out stack space after taking the task doesnt allow the stack to grow
    let Some(next) = next_task() else {
        todo!("no tasks, shutdown");
    };
    let Some(current) = swap_current(None) else {
        unreachable!("cannot stop a task that doesn't exist")
    };
    current.info.state.store(TaskState::Dropping);

    let context = current.context.get();

    // push the current thread to the drop queue AFTER switching
    after().push(CleanupTask::Drop(current));

    // SAFETY: `current` is stored in the queue until the switch
    // and the boxed field `context` makes sure the context pointer doesn't move
    unsafe {
        block(context, next);
    }

    unreachable!("a destroyed thread cannot continue executing");
}

pub fn spawn(f: impl FnOnce() + Send + 'static) {
    schedule(Task::new(f))
}

/// schedule
fn schedule(new: Task) {
    READY.push(new);
}

fn swap_current(mut new: Option<Task>) -> Option<Task> {
    swap(&mut new, &mut active());
    new
}

/// increase the task info's cpu usage field
fn update_cpu_usage(t: &Task) {
    let now = HPET.nanos() as u64;
    let last = last_time().swap(now, Ordering::SeqCst);

    let elapsed = now - last;

    t.info.nanos.fetch_add(elapsed, Ordering::Relaxed);
}

/// # Safety
///
/// `current` must be correct and point to a valid exclusive [`Context`]
unsafe fn block(current: *mut Context, next: Task) {
    next.info.state.store(TaskState::Running);

    let context = next.context.get();

    after().push(CleanupTask::Next(next));

    // SAFETY: `next` is stored in the queue until the switch
    // and the boxed field `context` makes sure the context pointer doesn't move
    unsafe {
        hyperion_arch::context::switch(current, context);
    }

    cleanup();
}

fn next_task() -> Option<Task> {
    READY.pop()
}

fn cleanup() {
    let after = after();

    while let Some(next) = after.pop() {
        match next {
            CleanupTask::Ready(ready) => {
                schedule(ready);
            }
            CleanupTask::Next(next) => {
                swap_current(Some(next));
            }
            CleanupTask::Drop(drop) => {
                if Arc::strong_count(&drop.memory) != 1 {
                    continue;
                }

                error!("TODO: deallocate task pages");

                // TODO: deallocate user pages
                // task.address_space;
            }
        };
    }
}

struct SchedulerTls {
    active: Mutex<Option<Task>>,
    after: SegQueue<CleanupTask>,
    last_time: AtomicU64,
}

static TLS: Lazy<Tls<SchedulerTls>> = Lazy::new(|| {
    Tls::new(|| SchedulerTls {
        active: Mutex::new(None),
        after: SegQueue::new(),
        last_time: AtomicU64::new(0),
    })
});

fn active() -> impl DerefMut<Target = Option<Task>> {
    TLS.active.lock()
}

fn after() -> &'static SegQueue<CleanupTask> {
    &TLS.after
}

fn last_time() -> &'static AtomicU64 {
    &TLS.last_time
}

fn page_fault_handler(addr: usize, user: Privilege) -> PageFaultResult {
    // hyperion_log::debug!("scheduler page fault");

    let active = active();
    let Some(current) = active.as_ref() else {
        hyperion_log::error!("page fault from kernel code");
        return PageFaultResult::NotHandled;
    };
    /* let current: *const Arc<TaskMemory> = CURRENT_ADDRESS_SPACE.as_ptr();
    let current = unsafe { (*current).clone() };
    assert_eq!(current.dbg_magic_byte, *MAGIC_DEBUG_BYTE); */

    if user == Privilege::User {
        user_page_fault_handler(addr, &current.memory)
    } else {
        kernel_page_fault_handler(addr, &current.memory)
    }
}

fn user_page_fault_handler(addr: usize, task: &TaskMemory) -> PageFaultResult {
    let result = task
        .user_stack
        .lock()
        .page_fault(&task.address_space.page_map, addr as u64);

    if result == PageFaultResult::Handled {
        return result;
    }

    hyperion_log::debug!("killing user-space process");
    stop();
}

fn kernel_page_fault_handler(addr: usize, task: &TaskMemory) -> PageFaultResult {
    let result = task
        .kernel_stack
        .lock()
        .page_fault(&task.address_space.page_map, addr as u64);

    if result == PageFaultResult::Handled {
        return result;
    }

    hyperion_log::error!("{:?}", task.kernel_stack.lock());
    hyperion_log::error!("page fault from kernel-space");
    result
}

fn take_job() -> Option<Box<dyn FnOnce() + Send + 'static>> {
    let mut active = active();
    active.as_mut()?.job.take()
}

extern "sysv64" fn thread_entry() -> ! {
    cleanup();
    (take_job().expect("no active jobs"))();
    stop();
}
