#![no_std]
#![feature(new_uninit, type_name_of_val, extract_if, sync_unsafe_cell, offset_of)]
#![allow(clippy::needless_return)]

//

use alloc::sync::Arc;
use core::{
    convert::Infallible,
    mem::{offset_of, swap},
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
};

use arcstr::ArcStr;
use crossbeam_queue::SegQueue;
use hyperion_arch::{cpu::ints, int};
use hyperion_cpu_id::Tls;
use hyperion_driver_acpi::{apic, hpet::HPET};
use hyperion_instant::Instant;
use hyperion_timer as timer;
use spin::{Lazy, Mutex, Once};
use time::Duration;
use x86_64::VirtAddr;

use self::{
    cleanup::{Cleanup, CleanupTask},
    task::{Pid, Process, Task, TaskState},
};
use crate::task::{switch_because, TaskInner};

//

extern crate alloc;

pub mod cleanup;
pub mod ipc;
// pub mod mpmc;
pub mod futex;
pub mod lock;
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
    Tls::inner(&TLS).iter().map(|tls| {
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
pub fn init(task: impl Into<Task>) -> ! {
    hyperion_arch::int::disable();

    // init scheduler's custom page fault handler
    ints::PAGE_FAULT_HANDLER.store(page_fault::page_fault_handler);

    // init scheduler's custom general protection fault handler
    ints::GP_FAULT_HANDLER.store(|| {
        process().should_terminate.store(true, Ordering::Relaxed);
        hyperion_log::debug!("GPF self term");
        stop();
    });

    // init HPET timer interrupts for sleep events
    timer::TIMER_HANDLER.store(|| sleep::wake_up_completed(None));

    // init periodic APIC timer interrutpts (optionally for RR-scheduling)
    apic::APIC_TIMER_HANDLER.store(|| {
        sleep::wake_up_completed(None);

        if !TLS.initialized.load(Ordering::SeqCst) {
            return;
        }

        if process().should_terminate.load(Ordering::Relaxed) {
            stop();
        }

        yield_now();
    });

    // hyperion_sync::init_futex(futex::wait, futex::wake);

    // init `Once` in TLS
    _ = crate::task();
    let task = task.into();

    // mark scheduler as initialized and running
    if TLS.initialized.swap(true, Ordering::SeqCst) {
        panic!("should be called only once before any tasks are assigned to this processor")
    }
    RUNNING.store(true, Ordering::SeqCst);

    // switch to the init task
    switch_because(task, TaskState::Dropping, Cleanup::Drop);
    unreachable!("a destroyed thread cannot continue executing");
}

pub fn send(target_pid: Pid, data: &[u8]) -> Result<(), &'static str> {
    ipc::pipe::send(target_pid, data)
}

pub fn recv(buf: &mut [u8]) -> Result<usize, &'static str> {
    ipc::pipe::recv(buf)
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

    let Ok(next) = wait_next_task(|| deadline.is_reached().then_some(())) else {
        return;
    };
    switch_because(next, TaskState::Sleeping, Cleanup::Sleep { deadline });
}

/// destroy the current thread
/// and switch to another thread
#[track_caller]
pub fn stop() -> ! {
    update_cpu_usage();

    let next = wait_next_task::<Infallible>(|| None).unwrap();
    switch_because(next, TaskState::Dropping, Cleanup::Drop);

    unreachable!("a destroyed thread cannot continue executing");
}

/// spawn a new thread in the currently running process
///
/// jumps into user space
pub fn spawn_userspace(fn_ptr: u64, fn_arg: u64) {
    spawn(move || {
        let stack_top = task().user_stack.lock().top;

        unsafe {
            hyperion_arch::syscall::userland(
                VirtAddr::new(fn_ptr),
                stack_top,
                stack_top.as_u64(),
                fn_arg,
            )
        };
    });
}

/// spawn a new process running this closure or a function or a task
pub fn schedule(new: impl Into<Task>) -> Pid {
    let task = new.into();
    let pid = task.pid;
    READY.push(task);
    pid
}

pub fn spawn(new: impl FnOnce() + Send + 'static) {
    READY.push(Task::thread(task(), new));
}

fn swap_current(mut new: Task) -> Task {
    swap(&mut new, &mut get_task().lock());
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

    task().nanos.fetch_add(elapsed, Ordering::Relaxed);
}

fn update_cpu_idle() {
    let elapsed = cpu_time_elapsed();

    idle_time().fetch_add(elapsed, Ordering::Relaxed);
}

fn wait_next_task<E>(mut should_abort: impl FnMut() -> Option<E>) -> Result<Task, E> {
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
    TLS.idle.store(true, Ordering::SeqCst);
    int::wait();
    TLS.idle.store(false, Ordering::SeqCst);
    update_cpu_idle();
}

fn cleanup() {
    let after = after();

    while let Some(next) = after.pop() {
        next.run();
    }
}

struct SchedulerTls {
    active: Once<Mutex<Task>>,
    after: SegQueue<CleanupTask>,
    last_time: AtomicU64,
    idle_time: AtomicU64,
    initialized: AtomicBool,
    idle: AtomicBool,

    switch_last_active: AtomicPtr<TaskInner>,
}

static TLS: Lazy<Tls<SchedulerTls>> = Lazy::new(|| {
    Tls::new(|| SchedulerTls {
        active: Once::new(),
        after: SegQueue::new(),
        last_time: AtomicU64::new(0),
        idle_time: AtomicU64::new(0),
        initialized: AtomicBool::new(false),
        idle: AtomicBool::new(false),

        switch_last_active: AtomicPtr::new(ptr::null_mut()),
    })
});

pub fn task() -> Task {
    (*get_task().lock()).clone()
}

pub fn process() -> Arc<Process> {
    get_task().lock().process.clone()
}

pub fn running() -> bool {
    // short circuits and doesnt init TLS unless it has to
    RUNNING.load(Ordering::SeqCst) && TLS.initialized.load(Ordering::SeqCst)
}

fn get_task() -> &'static Mutex<Task> {
    TLS.active.call_once(|| Mutex::new(Task::bootloader()))
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
