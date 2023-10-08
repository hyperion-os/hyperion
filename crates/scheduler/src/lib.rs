#![no_std]
#![feature(new_uninit, type_name_of_val, extract_if)]

//

use alloc::{
    boxed::Box,
    string::String,
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};
use core::{
    any::type_name_of_val,
    cell::UnsafeCell,
    fmt::Debug,
    mem::swap,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use crossbeam::atomic::AtomicCell;
use crossbeam_queue::SegQueue;
use hyperion_arch::{
    context::{switch as ctx_switch, Context},
    cpu::ints,
    int,
    stack::{AddressSpace, KernelStack, Stack, StackType, UserStack},
    tls::Tls,
    vmm::{PageMap, CURRENT_ADDRESS_SPACE},
};
use hyperion_driver_acpi::hpet::HPET;
use hyperion_instant::Instant;
use hyperion_log::*;
use hyperion_mem::{
    pmm::PFA,
    to_higher_half,
    vmm::{NotHandled, PageFaultResult, PageMapImpl, Privilege},
};
use hyperion_random::Rng;
use hyperion_timer::TIMER_HANDLER;
use spin::{Lazy, Mutex, MutexGuard, Once, RwLock};
use time::Duration;
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

static MAGIC_DEBUG_BYTE: Lazy<usize> = Lazy::new(|| hyperion_random::next_fast_rng().gen());

//

extern crate alloc;

pub mod executor;
pub mod keyboard;
pub mod process;
pub mod sleep;
pub mod task;
pub mod timer;

//

pub struct CleanupTask {
    task: Task,
    cleanup: Cleanup,
}

#[derive(Debug, Clone, Copy)]
pub enum Cleanup {
    Sleep { deadline: Instant },
    Drop,
    Ready,
}

impl Cleanup {
    pub const fn task(self, task: Task) -> CleanupTask {
        CleanupTask {
            task,
            cleanup: self,
        }
    }
}

pub struct Task {
    /// memory is per process
    pub memory: Arc<TaskMemory>,
    /// user_stack is per thread
    pub user_stack: Mutex<Stack<UserStack>>,

    // context is used 'unsafely' only in the switch
    context: Box<UnsafeCell<Context>>,
    job: Option<Box<dyn FnOnce() + Send + 'static>>,

    info: Arc<TaskInfo>,
}

pub struct TaskMemory {
    pub address_space: AddressSpace,
    pub kernel_stack: Mutex<Stack<KernelStack>>,
    dbg_magic_byte: usize,
}

#[derive(Debug)]
pub struct TaskInfo {
    // proc id
    pub pid: usize,

    // proc name
    pub name: RwLock<String>,

    // cpu time used
    pub nanos: AtomicU64,

    // proc state
    pub state: AtomicCell<TaskState>,
}

const _: () = assert!(AtomicCell::<TaskState>::is_lock_free());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Running,
    Sleeping,
    Ready,
    Dropping,
}

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

impl Task {
    pub fn new(f: impl FnOnce() + Send + 'static) -> Self {
        let name = type_name_of_val(&f);
        let job = Some(Box::new(f) as _);
        trace!("initializing task {name}");
        let name = RwLock::new(name.into());

        let info = Arc::new(TaskInfo {
            pid: Self::next_pid(),
            name,
            nanos: AtomicU64::new(0),
            state: AtomicCell::new(TaskState::Ready),
        });
        TASKS.lock().push(Arc::downgrade(&info));

        let address_space = AddressSpace::new(PageMap::new());

        let kernel_stack = address_space.take_kernel_stack_prealloc(1);
        let stack_top = kernel_stack.top;
        let main_thread_user_stack = address_space.take_user_stack_lazy();

        let memory = Arc::new(TaskMemory {
            address_space,
            kernel_stack: Mutex::new(kernel_stack),
            dbg_magic_byte: *MAGIC_DEBUG_BYTE,
        });

        let context = Box::new(UnsafeCell::new(Context::new(
            &memory.address_space.page_map,
            stack_top,
            thread_entry,
        )));

        let alloc = PFA.alloc(1);
        memory.address_space.page_map.map(
            CURRENT_ADDRESS_SPACE..CURRENT_ADDRESS_SPACE + 0xFFFu64,
            alloc.physical_addr(),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );
        let current_address_space: *mut Arc<TaskMemory> =
            to_higher_half(alloc.physical_addr()).as_mut_ptr();
        unsafe { current_address_space.write(memory.clone()) };

        Self {
            memory,
            user_stack: Mutex::new(main_thread_user_stack),

            context,
            job,
            info,
        }
    }

    /// # Safety
    ///
    /// this task is not safe to switch to
    unsafe fn bootloader() -> Self {
        // TODO: dropping this task should also free the bootloader stacks
        // they are currently wasting 64KiB per processor

        let info = Arc::new(TaskInfo {
            pid: 0,
            name: RwLock::new("bootloader".into()),
            nanos: AtomicU64::new(0),
            state: AtomicCell::new(TaskState::Ready),
        });

        let address_space = AddressSpace::new(PageMap::current());
        let kernel_stack = address_space.kernel_stacks.take();
        let user_stack = address_space.user_stacks.take();

        // SAFETY: covered by this function's safety doc
        let ctx = unsafe { Context::invalid(&address_space.page_map) };
        let context = Box::new(UnsafeCell::new(ctx));

        let memory = Arc::new(TaskMemory {
            address_space,
            kernel_stack: Mutex::new(kernel_stack),
            dbg_magic_byte: 0,
        });

        Self {
            memory,

            user_stack: Mutex::new(user_stack),

            context,
            job: None,
            info,
        }
    }

    pub fn next_pid() -> usize {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(1);
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

    pub fn ctx(&self) -> *mut Context {
        self.context.get()
    }

    pub fn set_state(&self, state: TaskState) {
        self.info.state.store(state);
    }
}

impl<F> From<F> for Task
where
    F: FnOnce() + Send + 'static,
{
    fn from(value: F) -> Self {
        Task::new(value)
    }
}

pub static READY: SegQueue<Task> = SegQueue::new();
pub static TASKS: Mutex<Vec<Weak<TaskInfo>>> = Mutex::new(vec![]);

pub fn task_memory() -> Arc<TaskMemory> {
    let current: *const Arc<TaskMemory> = CURRENT_ADDRESS_SPACE.as_ptr();
    let current = unsafe { (*current).clone() };
    assert_eq!(current.dbg_magic_byte, *MAGIC_DEBUG_BYTE);
    current
}

pub fn tasks() -> Vec<Arc<TaskInfo>> {
    let mut tasks = TASKS.lock();

    // remove tasks that don't exist
    tasks.retain(|p| p.strong_count() != 0);

    tasks.iter().filter_map(|p| p.upgrade()).collect()
}

pub fn idle() -> impl Iterator<Item = Duration> {
    Tls::inner(&TLS)
        .iter()
        .map(|tls| Duration::nanoseconds(tls.idle_time.load(Ordering::Relaxed) as _))
}

pub fn rename(new_name: String) {
    *active().info.name.write() = new_name;
}

/// init this processors scheduling
pub fn init() -> ! {
    hyperion_arch::int::disable();

    ints::PAGE_FAULT_HANDLER.store(page_fault_handler);
    TIMER_HANDLER.store(|| {
        for task in sleep::finished() {
            READY.push(task);
        }
    });
    hyperion_driver_acpi::apic::APIC_TIMER_HANDLER.store(|| {
        for task in sleep::finished() {
            warn!("TODO: fix APIC timer waking up HPET timers");
            READY.push(task);
        }

        // debug!("apic int");
        // hyperion_arch::dbg_cpu();

        // round-robin
        // yield_now();
    });

    if TLS.initialized.swap(true, Ordering::SeqCst) {
        panic!("should be called only once before any tasks are assigned to this processor")
    }

    stop();
}

/// switch to another thread
pub fn yield_now() {
    debug_assert!(TLS.initialized.load(Ordering::Relaxed));

    update_cpu_usage();

    let Some(next) = next_task() else {
        // no tasks -> keep the current task running
        return;
    };
    let next_ctx = next.ctx();
    next.set_state(TaskState::Running);

    let prev = swap_current(next);
    let prev_ctx = prev.ctx();
    prev.set_state(TaskState::Ready);

    // push the current thread back to the ready queue AFTER switching
    after().push(Cleanup::Ready.task(prev));

    // SAFETY: `prev` is stored in the queue, `next` is stored in the TLS
    // the box keeps the pointer pinned in memory
    unsafe { ctx_switch(prev_ctx, next_ctx) };

    cleanup();
}

pub fn sleep(duration: Duration) {
    sleep_until(Instant::now() + duration)
}

pub fn sleep_until(deadline: Instant) {
    debug_assert!(TLS.initialized.load(Ordering::Relaxed));

    update_cpu_usage();

    let Some(next) = wait_next_task_deadline(deadline) else {
        return;
    };
    let next_ctx = next.ctx();
    next.set_state(TaskState::Running);

    let prev = swap_current(next);
    let prev_ctx = prev.ctx();
    prev.set_state(TaskState::Sleeping);

    // push the current thread back to the ready queue AFTER switching
    after().push(Cleanup::Sleep { deadline }.task(prev));

    // SAFETY: `prev` is stored in the queue, `next` is stored in the TLS
    // the box keeps the pointer pinned in memory
    unsafe { ctx_switch(prev_ctx, next_ctx) };

    cleanup();
}

/// destroy the current thread
/// and switch to another thread
pub fn stop() -> ! {
    debug_assert!(TLS.initialized.load(Ordering::Relaxed));

    update_cpu_usage();

    // TODO: running out stack space after taking the task doesnt allow the stack to grow
    let next = wait_next_task();
    let next_ctx = next.ctx();
    next.set_state(TaskState::Running);

    let prev = swap_current(next);
    let prev_ctx = prev.ctx();
    prev.set_state(TaskState::Dropping);

    // push the current thread to the drop queue AFTER switching
    after().push(Cleanup::Drop.task(prev));

    // SAFETY: `prev` is stored in the queue, `next` is stored in the TLS
    // the box keeps the pointer pinned in memory
    unsafe { ctx_switch(prev_ctx, next_ctx) };

    unreachable!("a destroyed thread cannot continue executing");
}

/* pub fn spawn(f: impl FnOnce() + Send + 'static) {
    schedule(Task::new(f))
} */

/// spawn a new process running this closure or a function or a task
pub fn schedule(new: impl Into<Task>) {
    READY.push(new.into());
}

fn swap_current(mut new: Task) -> Task {
    let mut active = active();
    set_logger_task_name(Some(active.info.name.read().clone()));
    swap(&mut new, &mut active);
    new
}

#[must_use]
fn cpu_time_elapsed() -> u64 {
    let now = HPET.nanos() as u64;
    let last = last_time().swap(now, Ordering::SeqCst);

    now - last
}

fn reset_cpu_timer() {
    _ = cpu_time_elapsed();
}

/// increase the task info's cpu usage field
fn update_cpu_usage() {
    let elapsed = cpu_time_elapsed();

    active().info.nanos.fetch_add(elapsed, Ordering::Relaxed);
}

fn update_cpu_idle() {
    let elapsed = cpu_time_elapsed();

    idle_time().fetch_add(elapsed, Ordering::Relaxed);
}

fn wait_next_task() -> Task {
    loop {
        if let Some(task) = next_task() {
            return task;
        }

        // debug!("no tasks, waiting for interrupts");
        wait();
    }
}

fn wait_next_task_deadline(deadline: Instant) -> Option<Task> {
    loop {
        if let Some(task) = next_task() {
            return Some(task);
        }

        // debug!("no tasks, waiting for interrupts");
        wait();

        if deadline.is_reached() {
            return None;
        }
    }
}

fn next_task() -> Option<Task> {
    READY.pop()
}

fn wait() {
    reset_cpu_timer();
    int::wait();
    update_cpu_idle();
}

fn cleanup() {
    let after = after();

    while let Some(next) = after.pop() {
        let task = next.task;

        match next.cleanup {
            Cleanup::Ready => {
                schedule(task);
            }
            Cleanup::Sleep { deadline } => {
                sleep::push(deadline, task);
            }
            Cleanup::Drop => {
                if Arc::strong_count(&task.memory) != 1 {
                    continue;
                }

                trace!("TODO: deallocate task pages");

                // TODO: deallocate user pages
                // task.address_space;
            }
        };
    }
}

struct SchedulerTls {
    active: Once<Mutex<Task>>,
    after: SegQueue<CleanupTask>,
    last_time: AtomicU64,
    idle_time: AtomicU64,
    initialized: AtomicBool,
}

static TLS: Lazy<Tls<SchedulerTls>> = Lazy::new(|| {
    Tls::new(|| SchedulerTls {
        active: Once::new(),
        after: SegQueue::new(),
        last_time: AtomicU64::new(0),
        idle_time: AtomicU64::new(0),
        initialized: AtomicBool::new(false),
    })
});

pub fn active() -> MutexGuard<'static, Task> {
    TLS.active
        .call_once(|| Mutex::new(unsafe { Task::bootloader() }))
        .lock()
}

fn after() -> &'static SegQueue<CleanupTask> {
    &TLS.after
}

fn last_time() -> &'static AtomicU64 {
    &TLS.last_time
}

fn idle_time() -> &'static AtomicU64 {
    &TLS.idle_time
}

fn page_fault_handler(addr: usize, user: Privilege) -> PageFaultResult {
    hyperion_log::trace!("scheduler page fault (from {user:?})");

    if user == Privilege::User {
        let current = active();

        // `Err(Handled)` short circuits and returns
        handle_stack_grow(&current.user_stack, &current.memory, addr)?;

        // user process tried to access memory thats not available to it
        hyperion_log::warn!("killing user-space process");
        stop();
    } else {
        let current = task_memory();
        handle_stack_grow(&current.kernel_stack, &current, addr)?;

        let current = active();
        handle_stack_grow(&current.user_stack, &current.memory, addr)?;

        hyperion_log::error!("{:?}", current.memory.kernel_stack.lock());
        hyperion_log::error!("page fault from kernel-space");
    };

    let page = PageMap::current();
    let v = VirtAddr::new(addr as _);
    let p = page.virt_to_phys(v);
    error!("{v:018x?} -> {p:018x?}");

    Ok(NotHandled)
}

fn handle_stack_grow<T: StackType + Debug>(
    stack: &Mutex<Stack<T>>,
    task: &TaskMemory,
    addr: usize,
) -> PageFaultResult {
    stack
        .lock()
        .page_fault(&task.address_space.page_map, addr as u64)
}

extern "sysv64" fn thread_entry() -> ! {
    cleanup();
    let job = active().job.take().expect("no active jobs");
    job();
    stop();
}
