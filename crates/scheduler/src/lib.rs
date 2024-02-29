#![no_std]
#![feature(let_chains)]

//

use alloc::sync::Arc;
use core::{
    any::type_name_of_val,
    cell::{Cell, UnsafeCell},
    convert::Infallible,
    mem::{offset_of, swap},
    ops::Deref,
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
};

use arcstr::ArcStr;
use crossbeam_queue::SegQueue;
use hyperion_arch::{cpu::ints, int, stack::AddressSpace, vmm::PageMap};
use hyperion_cpu_id::Tls;
use hyperion_driver_acpi::{apic, hpet::HPET};
use hyperion_instant::Instant;
use hyperion_log::*;
use hyperion_mem::vmm::PageMapImpl;
use spin::{Mutex, Once};
use time::Duration;
use x86_64::VirtAddr;

use crate::{
    cleanup::{Cleanup, CleanupTask},
    proc::{Pid, Process},
    task::{switch_because, Task, TaskInner, TaskState},
};

//

extern crate alloc;

pub mod cleanup;
pub mod ipc;
// pub mod mpmc;
pub mod condvar;
pub mod futex;
pub mod lock;
pub mod proc;
pub mod sleep;
pub mod task;

mod page_fault;

//

// /// `T` is extra process data
// pub struct Scheduler<ProcExt, TaskExt> {
//     ready: SegQueue<Task<ProcExt, TaskExt>>,
// }

// impl<P, T> Scheduler<P, T> {
//     pub const fn new() -> Self {
//         Self {
//             ready: SegQueue::new(),
//         }
//     }
// }

// pub static RUNNING: Lazy<Tls<AtomicBool>> = Lazy::new(|| Tls::new(|| AtomicBool::new(false)));

pub static READY: SegQueue<Task> = SegQueue::new();
pub static RUNNING: AtomicBool = AtomicBool::new(false);

//

pub fn idle() -> impl Iterator<Item = Duration> {
    tls_iter().map(|tls| {
        fn _assert_sync<T: Sync>(_: T) {}
        fn _assert(tls: SchedulerTls) {
            _assert_sync(tls.idle_time);
        }

        let tls = tls.get();
        // SAFETY: idle_time field is Sync
        let idle_time =
            unsafe { &*((tls as usize + offset_of!(SchedulerTls, idle_time)) as *const AtomicU64) };
        // let idle_time = &unsafe { &*tls }.idle_time;

        Duration::nanoseconds(idle_time.load(Ordering::Relaxed) as _)
    })
}

pub fn rename(new_name: impl Into<ArcStr>) {
    *process().name.write() = new_name.into();
}

/// init this processors scheduling and
/// immediately switch to the provided task
pub fn init(thread: impl FnOnce() + Send + 'static) -> ! {
    // init the TLS struct before the apic timer handler tries to
    _ = tls();

    // init scheduler's custom page fault handler
    ints::PAGE_FAULT_HANDLER.store(page_fault::page_fault_handler);

    // init scheduler's custom general protection fault handler
    ints::GP_FAULT_HANDLER.store(|| {
        debug!("GPF self term");
        exit();
    });

    // init periodic APIC timer interrutpts (optionally for RR-scheduling)
    apic::APIC_TIMER_HANDLER.store(|| {
        hyperion_events::timer::wake();

        if tls().idle.load(Ordering::Acquire) {
            // don't task switch while waiting for tasks
            return;
        }

        // inter-processor interrupts instead
        if process().should_terminate.load(Ordering::Relaxed) {
            exit();
        }

        // yield_now();
    });

    // init `Once` in TLS
    _ = crate::task();

    static INIT: Once<Arc<Process>> = Once::new();
    let init = INIT
        .call_once(|| {
            let process = Process::new(
                Pid::next(),
                type_name_of_val(&thread).into(),
                AddressSpace::new(PageMap::new()),
            );
            process.threads.store(0, Ordering::Release);
            process
        })
        .clone();

    let task = Task::thread(init, thread);

    // mark scheduler as initialized and running
    if tls().initialized.swap(true, Ordering::SeqCst) {
        panic!("should be called only once before any tasks are assigned to this processor")
    }
    RUNNING.store(true, Ordering::SeqCst);

    // switch to the init task
    switch_because(task, TaskState::Dropping, Cleanup::Drop);
    unreachable!("a destroyed thread cannot continue executing");
}

/// switch to another thread
pub fn yield_now() {
    update_cpu_usage();

    let Some(next) = next_task() else {
        // no tasks -> keep the current task running
        return;
    };
    switch_because(next, TaskState::Ready, Cleanup::Ready);
}

pub fn yield_now_wait() {
    update_cpu_usage();

    let Some(next) = next_task() else {
        wait();
        // no tasks -> keep the current task running
        return;
    };
    switch_because(next, TaskState::Ready, Cleanup::Ready);
}

pub fn sleep(duration: Duration) {
    sleep_until(Instant::now() + duration)
}

pub fn sleep_until(deadline: Instant) {
    update_cpu_usage();

    let Ok(next) = wait_next_task_while(|| deadline.is_reached().then_some(())) else {
        return;
    };
    switch_because(next, TaskState::Sleeping, Cleanup::Sleep { deadline });
}

/// destroy the current thread
/// and switch to another thread
pub fn done() -> ! {
    update_cpu_usage();

    early_close_last_thread();

    switch_because(wait_next_task(), TaskState::Dropping, Cleanup::Drop);
    unreachable!("a destroyed thread cannot continue executing");
}

/// destroy the current process
/// and switch to another
pub fn exit() -> ! {
    update_cpu_usage();

    // FIXME: trigger an IPI on all cpu's running for this process
    process().should_terminate.store(true, Ordering::Release);

    force_close_thread();

    switch_because(wait_next_task(), TaskState::Dropping, Cleanup::Drop);
    unreachable!("a destroyed thread cannot continue executing");
}

/// spawn a new thread in the currently running process
///
/// jumps into user space
pub fn spawn_userspace(fn_ptr: u64, fn_arg: u64) {
    spawn(move || {
        let stack_top = task().user_stack.lock().top.align_down(0x10u64) - 0x8u64;

        hyperion_arch::syscall::userland(
            VirtAddr::new(fn_ptr),
            stack_top,
            stack_top.as_u64(),
            fn_arg,
        );
    });
}

/// fork the active process
pub fn fork(f: impl FnOnce() + Send + 'static) -> Pid {
    update_cpu_usage();
    schedule(task().fork(f))
}

/// spawn a new process running this closure or a function or a task
pub fn schedule(new: impl Into<Task>) -> Pid {
    let task = new.into();
    let pid = task.pid;
    READY.push(task);
    pid
}

/// spawn a new thread on the same process
pub fn spawn(new: impl FnOnce() + Send + 'static) {
    READY.push(Task::thread(process(), new));
}

fn force_close_thread() {
    if let Some(ext) = process().ext.get() {
        ext.close();
    }
}

fn early_close_last_thread() {
    let proc = process();
    if let Some(ext) = proc.ext.get()
        && proc.threads.load(Ordering::SeqCst) == 1
    {
        ext.close();
    }
}

fn swap_current(mut new: Task) -> Task {
    swap(&mut new, &mut get_task().lock());
    new
}

#[must_use]
fn cpu_time_elapsed() -> u64 {
    let now = HPET.nanos() as u64;
    let last = last_time().swap(now, Ordering::SeqCst);

    now.saturating_sub(last)
}

fn reset_cpu_timer() {
    _ = cpu_time_elapsed();
}

/// increase the task info's cpu usage field
fn update_cpu_usage() {
    let elapsed = cpu_time_elapsed();

    task().nanos.fetch_add(elapsed, Ordering::Relaxed);
}

fn update_cpu_idle() {
    let elapsed = cpu_time_elapsed();

    idle_time().fetch_add(elapsed, Ordering::Relaxed);
}

// take the next ready task
//
// if no tasks are available, start sleeping
//
// won't give up
fn wait_next_task() -> Task {
    wait_next_task_while::<Infallible>(|| None).unwrap_or_else(|e| match e {})
}

// take the next ready task
//
// if no tasks are available, start sleeping
//
// gives up if `should_abort` returns true
fn wait_next_task_while<E>(mut should_abort: impl FnMut() -> Option<E>) -> Result<Task, E> {
    update_cpu_usage();

    loop {
        if let Some(task) = next_task() {
            return Ok(task);
        }

        // debug!("no tasks, waiting for interrupts");
        wait();

        if let Some(err) = should_abort() {
            return Err(err);
        }
    }
}

fn next_task() -> Option<Task> {
    READY.pop()
}

fn wait() {
    reset_cpu_timer();
    tls().idle.store(true, Ordering::SeqCst);
    int::wait();
    tls().idle.store(false, Ordering::SeqCst);
    update_cpu_idle();
}

// post context switch jobs
fn cleanup() {
    if let Some(cleanup) = tls().take_cleanup_task() {
        cleanup.run();
    }
}

/// per CPU (a physical CPU, not a thread/task) scheduling info,
/// each CPU has to know the task it is working on and other stuff
struct SchedulerTls {
    active: Once<Mutex<Task>>,
    after: Cell<Option<CleanupTask>>,
    last_time: AtomicU64,
    idle_time: AtomicU64,
    initialized: AtomicBool,
    idle: AtomicBool,

    switch_last_active: AtomicPtr<TaskInner>,
}

impl SchedulerTls {
    fn set_cleanup_task(&self, task: CleanupTask) {
        let old = self.after.replace(Some(task));
        debug_assert!(old.is_none());
    }

    fn take_cleanup_task(&self) -> Option<CleanupTask> {
        self.after.take()
    }
}

static TLS: Once<Tls<SchedulerTls>> = Once::new();

fn tls() -> &'static Tls<SchedulerTls> {
    TLS.call_once(|| {
        Tls::new(|| SchedulerTls {
            active: Once::new(),
            after: Cell::new(None),
            last_time: AtomicU64::new(0),
            idle_time: AtomicU64::new(0),
            initialized: AtomicBool::new(false),
            idle: AtomicBool::new(false),

            switch_last_active: AtomicPtr::new(ptr::null_mut()),
        })
    })
}

fn tls_iter() -> impl Iterator<Item = &'static UnsafeCell<SchedulerTls>> {
    Tls::inner(tls()).iter()
}

fn tls_try() -> Option<&'static SchedulerTls> {
    TLS.get().map(|s| s.deref())
}

pub fn task() -> Task {
    (*get_task().lock()).clone()
}

pub fn process() -> Arc<Process> {
    get_task().lock().process.clone()
}

pub fn running() -> bool {
    // short circuits and doesnt init TLS unless it has to
    RUNNING.load(Ordering::SeqCst)
        && tls_try()
            .map(|v| v.initialized.load(Ordering::SeqCst))
            .unwrap_or(false)
}

fn get_task() -> &'static Mutex<Task> {
    tls().active.call_once(|| Mutex::new(Task::bootloader()))
}

fn last_time() -> &'static AtomicU64 {
    &tls().last_time
}

fn idle_time() -> &'static AtomicU64 {
    &tls().idle_time
}
