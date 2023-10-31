use alloc::{
    borrow::Cow,
    boxed::Box,
    collections::BTreeMap,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    any::type_name_of_val,
    cell::UnsafeCell,
    fmt,
    ops::Deref,
    ptr,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use crossbeam::atomic::AtomicCell;
use hyperion_arch::{
    context::{switch as ctx_switch, Context},
    stack::{AddressSpace, KernelStack, Stack, UserStack},
    vmm::PageMap,
};
use hyperion_bitmap::Bitmap;
use hyperion_log::*;
use hyperion_mem::{pmm, vmm::PageMapImpl};
use spin::{Mutex, MutexGuard, Once, RwLock};

use crate::{after, cleanup::Cleanup, ipc::SimpleIpc, stop, swap_current, task, TakeOnce, TLS};

//

// static MAGIC_DEBUG_BYTE: Lazy<usize> = Lazy::new(|| hyperion_random::next_fast_rng().gen());

// TODO: get rid of the slow dumbass spinlock mutexes everywhere
pub static PROCESSES: Mutex<BTreeMap<Pid, Weak<Process>>> = Mutex::new(BTreeMap::new());
// pub static TASKS: Mutex<Vec<Weak<Process>>> = Mutex::new(Vec::new());

pub static TASKS_RUNNING: AtomicUsize = AtomicUsize::new(0);
pub static TASKS_SLEEPING: AtomicUsize = AtomicUsize::new(0);
pub static TASKS_READY: AtomicUsize = AtomicUsize::new(0);
pub static TASKS_DROPPING: AtomicUsize = AtomicUsize::new(0);

//

pub fn processes() -> Vec<Arc<Process>> {
    let processes = PROCESSES.lock();
    // processes.retain(|_, proc| proc.upgrade().is_some());

    processes
        .values()
        .filter_map(|proc| proc.upgrade())
        .collect()
}

pub fn switch_because(next: Task, new_state: TaskState, cleanup: Cleanup) {
    // debug!("switching to {}", next.name.read().clone());
    if !next.is_valid {
        panic!("this task is not safe to switch to");
    }

    let next_ctx = next.context.get();
    if next.swap_state(TaskState::Running) == TaskState::Running {
        panic!("this task is already running");
    }

    // tell the page fault handler that the actual current task is still this one
    let task = task();
    let task_inner: &TaskInner = &task;
    TLS.switch_last_active.store(
        task_inner as *const TaskInner as *mut TaskInner,
        Ordering::SeqCst,
    );
    drop(task);

    let prev = swap_current(next);
    let prev_ctx = prev.context.get();
    if prev.swap_state(new_state) != TaskState::Running {
        panic!("previous task wasn't running");
    }

    // push the current thread to the drop queue AFTER switching
    after().push(cleanup.task(prev));

    // SAFETY: `prev` is stored in the queue, `next` is stored in the TLS
    // the box keeps the pointer pinned in memory
    debug_assert!(TLS.initialized.load(Ordering::Relaxed));
    unsafe { ctx_switch(prev_ctx, next_ctx) };

    // the ctx_switch can continue either in `thread_entry` or here:

    post_ctx_switch();
}

fn post_ctx_switch() {
    // invalidate the page fault handler's old task store
    TLS.switch_last_active
        .store(ptr::null_mut(), Ordering::SeqCst);

    crate::cleanup();
}

extern "C" fn thread_entry() -> ! {
    post_ctx_switch();

    let job = task().job.take().expect("no active jobs");
    job();

    stop();
}

//

#[derive(Debug, Default)]
pub struct PageAllocs {
    // TODO: a better way to keep track of allocated pages
    bitmap: Once<Mutex<Bitmap<'static>>>,
}

impl PageAllocs {
    pub fn bitmap(&self) -> MutexGuard<Bitmap<'static>> {
        self.bitmap
            .call_once(|| {
                let bitmap_alloc: Vec<u8> = (0..pmm::PFA.bitmap_len() / 8).map(|_| 0u8).collect();
                let bitmap_alloc = Vec::leak(bitmap_alloc); // TODO: free
                Mutex::new(Bitmap::new(bitmap_alloc))
            })
            .lock()
    }
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid(usize);

impl Pid {
    pub const fn new(num: usize) -> Self {
        Self(num)
    }

    pub fn next() -> Self {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(1);
        Pid(NEXT_PID.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn num(self) -> usize {
        self.0
    }
}

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

//

/// A process, each process can have multiple 'tasks' (pthreads)
pub struct Process {
    /// process id
    pub pid: Pid,

    /// process name
    pub name: RwLock<Cow<'static, str>>,

    /// cpu time this process (all tasks) has used in nanoseconds
    pub nanos: AtomicU64,

    /// process address space
    pub address_space: AddressSpace,

    /// process heap beginning, the end of the user process
    pub heap_bottom: AtomicUsize,

    /// naïve PID based IPC
    pub simple_ipc: SimpleIpc,

    /// a store for all allocated (and mapped) physical pages
    pub allocs: PageAllocs,

    pub should_terminate: AtomicBool,
}

impl Drop for Process {
    fn drop(&mut self) {
        PROCESSES.lock().remove(&self.pid);
    }
}

//

pub struct TaskInner {
    /// a shared process ref, multiple tasks can point to the same process
    pub process: Arc<Process>,

    /// task state, 'is the task waiting or what?'
    pub state: AtomicCell<TaskState>,

    /// lazy initialized user-space stack
    pub user_stack: Mutex<Stack<UserStack>>,

    /// lazy initialized kernel-space stack,
    /// also used when entering kernel-space from a `syscall` but not from a interrupt
    /// each CPU has its privilege stack in TSS, page faults also have their own stacks per CPU
    pub kernel_stack: Mutex<Stack<KernelStack>>,

    /// thread_entry runs this function once, and stops the process after returning
    pub job: TakeOnce<Box<dyn FnOnce() + Send + 'static>>,

    // context is used 'unsafely' only in the switch
    // TaskInner is pinned in heap using a Box to make sure a pointer to this (`context`)
    // is valid after switching task before switching context
    context: UnsafeCell<Context>,

    // context is valid to switch to only if this is true
    is_valid: bool,
}

impl Deref for TaskInner {
    type Target = Process;

    fn deref(&self) -> &Self::Target {
        &self.process
    }
}

unsafe impl Sync for TaskInner {}

impl Drop for TaskInner {
    #[track_caller]
    fn drop(&mut self) {
        assert_eq!(
            self.state.load(),
            TaskState::Dropping,
            "{} {}",
            self.name.read().clone(),
            core::panic::Location::caller(),
        );
        TASKS_DROPPING.fetch_sub(1, Ordering::Relaxed);

        // TODO: drop pages

        // SAFETY: self.memory is not used anymore
        // let memory = unsafe { ManuallyDrop::take(&mut self.memory) };

        // if Arc::into_inner(memory).is_some() {
        //     TASK_MEM.lock().remove(&self.info.pid);
        // }
    }
}

//

#[derive(Clone)]
pub struct Task(Arc<TaskInner>);

impl Task {
    pub fn new(f: impl FnOnce() + Send + 'static) -> Task {
        let name = type_name_of_val(&f);
        Self::new_any(Box::new(f) as _, name.into())
    }

    pub fn new_any(f: Box<dyn FnOnce() + Send + 'static>, name: Cow<'static, str>) -> Task {
        trace!("initializing task {name}");

        let process = Arc::new(Process {
            pid: Pid::next(),
            name: RwLock::new(name),
            nanos: AtomicU64::new(0),
            address_space: AddressSpace::new(PageMap::new()),
            heap_bottom: AtomicUsize::new(0x1000),
            simple_ipc: SimpleIpc::new(),
            allocs: PageAllocs::default(),
            should_terminate: AtomicBool::new(false),
        });
        PROCESSES
            .lock()
            .insert(process.pid, Arc::downgrade(&process));

        let kernel_stack = process.address_space.take_kernel_stack_prealloc(1);
        let stack_top = kernel_stack.top;
        let user_stack = process.address_space.take_user_stack_lazy();

        let context = UnsafeCell::new(Context::new(
            &process.address_space.page_map,
            stack_top,
            thread_entry,
        ));

        trace!(
            "task rsp:{stack_top:#x} cr3:{:#x}",
            process.address_space.page_map.cr3().start_address()
        );

        TASKS_READY.fetch_add(1, Ordering::Relaxed);
        Self(Arc::new(TaskInner {
            process,
            state: AtomicCell::new(TaskState::Ready),
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
            job: TakeOnce::new(f),
            context,
            is_valid: true,
        }))
    }

    pub fn thread(this: Task, f: impl FnOnce() + Send + 'static) -> Task {
        Self::thread_any(this, Box::new(f))
    }

    pub fn thread_any(this: Task, f: Box<dyn FnOnce() + Send + 'static>) -> Task {
        trace!(
            "initializing secondary thread for process {}",
            this.name.read().clone()
        );

        let process = this.process.clone();

        let kernel_stack = process.address_space.take_kernel_stack_prealloc(1);
        let stack_top = kernel_stack.top;
        let user_stack = process.address_space.take_user_stack_lazy();

        let context = UnsafeCell::new(Context::new(
            &process.address_space.page_map,
            stack_top,
            thread_entry,
        ));

        TASKS_READY.fetch_add(1, Ordering::Relaxed);
        Self(Arc::new(TaskInner {
            process,
            state: AtomicCell::new(TaskState::Ready),
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
            job: TakeOnce::new(f),
            context,
            is_valid: true,
        }))
    }

    pub fn bootloader() -> Task {
        // TODO: dropping this task should also free the bootloader stacks
        // they are currently dropped by a task in kernel/src/main.rs

        trace!("initializing bootloader task");

        let process = Arc::new(Process {
            pid: Pid(0),
            name: RwLock::new("bootloader".into()),
            nanos: AtomicU64::new(0),
            address_space: AddressSpace::new(PageMap::current()),
            heap_bottom: AtomicUsize::new(0x1000),
            simple_ipc: SimpleIpc::new(),
            allocs: PageAllocs::default(),
            should_terminate: AtomicBool::new(true),
        });

        let mut kernel_stack = process
            .address_space
            .kernel_stacks
            .take_lazy(&process.address_space.page_map);
        let mut user_stack = process
            .address_space
            .user_stacks
            .take_lazy(&process.address_space.page_map);
        kernel_stack.limit_4k_pages = 0;
        user_stack.limit_4k_pages = 0;

        // SAFETY: this task is unsafe to switch to,
        // switching is allowed only if `self.is_valid()` returns true
        let context = UnsafeCell::new(unsafe { Context::invalid(&process.address_space.page_map) });

        TASKS_RUNNING.fetch_add(1, Ordering::Relaxed);
        Self(Arc::new(TaskInner {
            process,
            state: AtomicCell::new(TaskState::Running),
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
            job: TakeOnce::none(),
            context,
            is_valid: false,
        }))
    }

    pub fn swap_state(&self, new: TaskState) -> TaskState {
        match new {
            TaskState::Running => &TASKS_RUNNING,
            TaskState::Sleeping => &TASKS_SLEEPING,
            TaskState::Ready => &TASKS_READY,
            TaskState::Dropping => &TASKS_DROPPING,
        }
        .fetch_add(1, Ordering::Relaxed);

        let old = self.state.swap(new);

        match old {
            TaskState::Running => &TASKS_RUNNING,
            TaskState::Sleeping => &TASKS_SLEEPING,
            TaskState::Ready => &TASKS_READY,
            TaskState::Dropping => &TASKS_DROPPING,
        }
        .fetch_sub(1, Ordering::Relaxed);

        old
    }
}

impl Deref for Task {
    type Target = TaskInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<F> From<F> for Task
where
    F: FnOnce() + Send + 'static,
{
    fn from(value: F) -> Self {
        Self::new(value)
    }
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Running,
    Sleeping,
    Ready,
    Dropping,
}

const _: () = assert!(AtomicCell::<TaskState>::is_lock_free());

impl TaskState {
    pub const fn as_str(self) -> &'static str {
        match self {
            TaskState::Running => "running",
            TaskState::Sleeping => "sleeping",
            TaskState::Ready => "ready",
            TaskState::Dropping => "dropping",
        }
    }

    /// Returns `true` if the task state is [`Running`].
    ///
    /// [`Running`]: TaskState::Running
    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Returns `true` if the task state is [`Sleeping`].
    ///
    /// [`Sleeping`]: TaskState::Sleeping
    #[must_use]
    pub fn is_sleeping(&self) -> bool {
        matches!(self, Self::Sleeping)
    }

    /// Returns `true` if the task state is [`Ready`].
    ///
    /// [`Ready`]: TaskState::Ready
    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Returns `true` if the task state is [`Dropping`].
    ///
    /// [`Dropping`]: TaskState::Dropping
    #[must_use]
    pub fn is_dropping(&self) -> bool {
        matches!(self, Self::Dropping)
    }
}
