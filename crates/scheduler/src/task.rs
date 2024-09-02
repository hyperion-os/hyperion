use alloc::{boxed::Box, sync::Arc};
use core::{
    any::type_name_of_val,
    cell::UnsafeCell,
    fmt, mem,
    ops::Deref,
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

use arcstr::ArcStr;
use crossbeam::atomic::AtomicCell;
use hyperion_arch::{
    context::{switch as ctx_switch, Context},
    stack::{AddressSpace, KernelStack, Stack, UserStack},
    vmm::PageMap,
};
use hyperion_log::*;
use hyperion_mem::vmm::PageMapImpl;
use hyperion_sync::TakeOnce;
use spin::{Mutex, Once};
use x86_64::{
    align_up, registers::model_specific::FsBase, structures::paging::PageTableFlags, VirtAddr,
};

use crate::{
    cleanup::Cleanup,
    done,
    proc::{Pid, Process},
    swap_current, task, tls,
};

//

pub static TASKS_RUNNING: AtomicUsize = AtomicUsize::new(0);
pub static TASKS_SLEEPING: AtomicUsize = AtomicUsize::new(0);
pub static TASKS_READY: AtomicUsize = AtomicUsize::new(0);
pub static TASKS_DROPPING: AtomicUsize = AtomicUsize::new(0);

//

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
    {
        let task = task();
        let task_inner: &TaskInner = &task;
        tls().switch_last_active.store(
            task_inner as *const TaskInner as *mut TaskInner,
            Ordering::SeqCst,
        );
        drop(task);
    }

    let prev = swap_current(next);
    let prev_ctx = prev.context.get();
    if prev.swap_state(new_state) != TaskState::Running {
        panic!("previous task wasn't running");
    }

    // push the current thread to the drop queue AFTER switching
    tls().set_cleanup_task(cleanup.task(prev));

    // SAFETY: `prev` is stored in the queue, `next` is stored in the TLS
    // the box keeps the pointer pinned in memory
    debug_assert!(tls().initialized.load(Ordering::SeqCst));
    unsafe { ctx_switch(prev_ctx, next_ctx) };

    // the ctx_switch can continue either in `thread_entry` or here:

    post_ctx_switch();
}

fn post_ctx_switch() {
    // invalidate the page fault handler's old task store
    tls()
        .switch_last_active
        .store(ptr::null_mut(), Ordering::SeqCst);

    task().init_tls();

    crate::cleanup();

    // hyperion_arch::
}

extern "C" fn thread_entry() -> ! {
    post_ctx_switch();

    let job = task().job.take().expect("no active jobs");
    job();

    done();
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tid(usize);

impl Tid {
    pub const fn new(num: usize) -> Self {
        Self(num)
    }

    pub fn next(proc: &Process) -> Self {
        Self::new(proc.next_tid.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn num(self) -> usize {
        self.0
    }
}

impl fmt::Display for Tid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

//

pub struct TaskInner {
    /// thread id
    ///
    /// thread id's are per process, each process has at least TID 0
    pub tid: Tid,

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

    /// a copy of the master TLS for specifically this task
    pub tls: Once<VirtAddr>,

    // context is used 'unsafely' only in the switch
    // TaskInner is pinned in heap using a Box to make sure a pointer to this (`context`)
    // is valid after switching task before switching context
    context: UnsafeCell<Context>,

    // context is valid to switch to only if this is true
    is_valid: bool,
}

impl TaskInner {
    pub fn init_tls(&self) {
        let fs = self
            .tls
            .try_call_once(|| {
                let Some((addr, layout)) = self.master_tls.get().copied() else {
                    // master tls doesn't exist, so don't copy it
                    return Err(());
                };
                // debug!("init_tls for {}", self.name.read());

                // TODO: deallocate it when the thread quits

                // afaik, it has to be at least one page?
                // and at least 2 pages if TLS has any data
                // , because FSBase has to point to TCB and
                // FSBase has to be page aligned and TLS
                // has to be right below TCB
                // +--------------+--------------+
                // |   PHYS PAGE  |  PHYS PAGE   |
                // +--------+-----+-----+--------+
                // | unused | TLS | TCB | unused |
                // +--------+-----+-----+--------+

                // the alloc is align_up(TLS) + TCB
                let tls_alloc = align_up(layout.size() as u64, 0x10u64);
                let n_pages = tls_alloc.div_ceil(0x1000) as usize + 1;
                let tls_copy = self
                    .alloc(
                        n_pages,
                        PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE,
                    )
                    .unwrap();

                let tcb_ptr = tls_copy + (n_pages - 1) * 0x1000;
                let tls_ptr = tcb_ptr - tls_alloc;

                // copy the master TLS
                unsafe {
                    ptr::copy_nonoverlapping::<u8>(
                        addr.as_ptr(),
                        tls_ptr.as_mut_ptr(),
                        layout.size(),
                    );
                }

                // init TCB
                // %fs register value, it points to a pointer to itself, TLS items are right before it
                let fs = tcb_ptr;
                // %fs:0x0 should point to fsbase
                unsafe { *fs.as_mut_ptr() = fs.as_u64() };

                Ok(fs)
            })
            .ok()
            .copied()
            .unwrap_or(VirtAddr::new(0));

        // TODO: a more robust is_active fn
        let active = task();
        if self.tid == active.tid && self.pid == active.pid {
            // debug!("tls fs={fs:#018x}");
            FsBase::write(fs);
            // unsafe { FS::write_base(tls_copy) };
        }
    }
}

impl Deref for TaskInner {
    type Target = Process;

    fn deref(&self) -> &Self::Target {
        &self.process
    }
}

unsafe impl Sync for TaskInner {}

impl Drop for TaskInner {
    fn drop(&mut self) {
        assert_eq!(
            self.state.load(),
            TaskState::Dropping,
            "{}",
            self.name.read().clone(),
        );

        // hyperion_log::debug!("dropping task {:?} of '{}'", self.tid, self.name.read());

        self.threads.fetch_sub(1, Ordering::Relaxed);

        let k_stack = mem::take(&mut self.kernel_stack).into_inner();
        let u_stack = mem::take(&mut self.user_stack).into_inner();
        let spc = &self.address_space;
        spc.kernel_stacks.free(&spc.page_map, k_stack);
        spc.user_stacks.free(&spc.page_map, u_stack);

        TASKS_DROPPING.fetch_sub(1, Ordering::Relaxed);
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

    pub fn new_any(f: Box<dyn FnOnce() + Send + 'static>, name: ArcStr) -> Task {
        trace!("initializing task {name}");

        let process = Process::new(Pid::next(), name, AddressSpace::new(PageMap::new()));

        let kernel_stack = process.address_space.take_kernel_stack_prealloc(1);
        let user_stack = process.address_space.take_user_stack();

        let context = UnsafeCell::new(Context::new(
            &process.address_space.page_map,
            kernel_stack.top,
            thread_entry,
        ));

        TASKS_READY.fetch_add(1, Ordering::Relaxed);
        Self(Arc::new(TaskInner {
            tid: Tid::next(&process),
            process,
            state: AtomicCell::new(TaskState::Ready),
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
            job: TakeOnce::new(f),
            tls: Once::new(),
            context,
            is_valid: true,
        }))
    }

    pub fn fork(&self, f: impl FnOnce() + Send + 'static) -> Task {
        self.fork_any(Box::new(f))
    }

    pub fn fork_any(&self, f: Box<dyn FnOnce() + Send + 'static>) -> Task {
        let name = self.name.read().clone();
        trace!("initializing a fork of process {name}");

        let user_stack = self.user_stack.lock().clone();
        let address_space = self.address_space.fork(&user_stack);
        let kernel_stack = address_space.take_kernel_stack_prealloc(1);

        let context = UnsafeCell::new(Context::new(
            &address_space.page_map,
            kernel_stack.top,
            thread_entry,
        ));

        let process = Process::new(Pid::next(), name, address_space);

        TASKS_READY.fetch_add(1, Ordering::Relaxed);
        Self(Arc::new(TaskInner {
            tid: Tid::next(&process),
            process,
            state: AtomicCell::new(TaskState::Ready),
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
            job: TakeOnce::new(f),
            tls: Once::new(),
            context,
            is_valid: true,
        }))
    }

    pub fn thread(process: Arc<Process>, f: impl FnOnce() + Send + 'static) -> Task {
        Self::thread_any(process, Box::new(f))
    }

    pub fn thread_any(process: Arc<Process>, f: Box<dyn FnOnce() + Send + 'static>) -> Task {
        trace!(
            "initializing secondary thread for process {}",
            process.name.read().clone()
        );

        process.threads.fetch_add(1, Ordering::Relaxed);

        let kernel_stack = process.address_space.take_kernel_stack_prealloc(1);
        let user_stack = process.address_space.take_user_stack();

        let context = UnsafeCell::new(Context::new(
            &process.address_space.page_map,
            kernel_stack.top,
            thread_entry,
        ));

        TASKS_READY.fetch_add(1, Ordering::Relaxed);
        Self(Arc::new(TaskInner {
            tid: Tid::next(&process),
            process,
            state: AtomicCell::new(TaskState::Ready),
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
            job: TakeOnce::new(f),
            tls: Once::new(),
            context,
            is_valid: true,
        }))
    }

    pub fn bootloader() -> Task {
        // TODO: dropping this task should also free the bootloader stacks
        // they are currently dropped by a task in kernel/src/main.rs

        trace!("initializing bootloader task");

        let process = Process::new(
            Pid::new(0),
            "bootloader".into(),
            AddressSpace::new(PageMap::current()),
        );
        process.should_terminate.store(true, Ordering::Release);

        let mut kernel_stack = process
            .address_space
            .kernel_stacks
            .take(&process.address_space.page_map);
        let mut user_stack = process
            .address_space
            .user_stacks
            .take(&process.address_space.page_map);
        kernel_stack.limit_4k_pages = 0;
        user_stack.limit_4k_pages = 0;

        // SAFETY: this task is unsafe to switch to,
        // switching is allowed only if `self.is_valid()` returns true
        let context = UnsafeCell::new(unsafe { Context::invalid() });

        TASKS_RUNNING.fetch_add(1, Ordering::Relaxed);
        Self(Arc::new(TaskInner {
            tid: Tid::next(&process),
            process,
            state: AtomicCell::new(TaskState::Running),
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
            job: TakeOnce::none(),
            tls: Once::new(),
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

    // pub fn ptr_eq(&self, other: &Task) -> bool {
    //     Arc::ptr_eq(&self.0, &other.0)
    // }
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
