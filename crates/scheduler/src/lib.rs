#![no_std]
#![feature(new_uninit, type_name_of_val, extract_if, sync_unsafe_cell, offset_of)]
#![allow(clippy::needless_return)]

//

use alloc::{borrow::Cow, sync::Arc};
use core::{
    mem::{offset_of, swap},
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
};

use crossbeam_queue::SegQueue;
use hyperion_arch::{cpu::ints, int, tls::Tls};
use hyperion_driver_acpi::hpet::HPET;
use hyperion_instant::Instant;
use hyperion_log::*;
use hyperion_timer::TIMER_HANDLER;
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
pub mod sleep;
pub mod task;

mod page_fault;

//

pub static READY: SegQueue<Task> = SegQueue::new();
pub static RUNNING: AtomicBool = AtomicBool::new(false);

//

pub struct TakeOnce<T> {
    val: Mutex<Option<T>>,
    taken: AtomicBool,
}

impl<T> TakeOnce<T> {
    pub const fn new(val: T) -> Self {
        Self {
            val: Mutex::new(Some(val)),
            taken: AtomicBool::new(false),
        }
    }

    pub const fn none() -> Self {
        Self {
            val: Mutex::new(None),
            taken: AtomicBool::new(true),
        }
    }

    pub fn take(&self) -> Option<T> {
        if self.taken.swap(true, Ordering::AcqRel) {
            None
        } else {
            self.take_lock()
        }
    }

    #[cold]
    fn take_lock(&self) -> Option<T> {
        self.val.lock().take()
    }
}

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

pub fn rename(new_name: Cow<'static, str>) {
    *process().name.write() = new_name;
}

/// init this processors scheduling
pub fn init() -> ! {
    hyperion_arch::int::disable();

    ints::PAGE_FAULT_HANDLER.store(page_fault::page_fault_handler);
    TIMER_HANDLER.store(|| {
        sleep::wake_up_completed(None);
    });
    hyperion_driver_acpi::apic::APIC_TIMER_HANDLER.store(|| {
        sleep::wake_up_completed(None);

        if TLS.idle.load(Ordering::SeqCst) {
            return;
        }

        // debug!("apic int");
        // hyperion_arch::dbg_cpu();

        // round-robin
        // debug!("round-robin fake yield now");
        // yield_now();

        // TODO: test if the currently running task has used way too much cpu time and switch if so
    });

    _ = task();
    if TLS.initialized.swap(true, Ordering::SeqCst) {
        panic!("should be called only once before any tasks are assigned to this processor")
    }
    RUNNING.store(true, Ordering::SeqCst);

    stop();
}

pub fn send(target_pid: Pid, data: Cow<'static, [u8]>) -> Result<(), &'static str> {
    ipc::send(target_pid, data)
}

pub fn recv() -> Cow<'static, [u8]> {
    ipc::recv()
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
    wait();
    update_cpu_usage();

    let Some(next) = next_task() else {
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

    let Some(next) = wait_next_task(|| deadline.is_reached()) else {
        return;
    };
    switch_because(next, TaskState::Sleeping, Cleanup::Sleep { deadline });
}

/// destroy the current thread
/// and switch to another thread
pub fn stop() -> ! {
    update_cpu_usage();

    let next = wait_next_task(|| false).unwrap();
    switch_because(next, TaskState::Dropping, Cleanup::Drop);

    unreachable!("a destroyed thread cannot continue executing");
}

/// spawn a new thread in the currently running process
///
/// jumps into user space
pub fn spawn(fn_ptr: u64, fn_arg: u64) {
    let thread = Task::thread(task(), move || {
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
    READY.push(thread);
}
/// spawn a new process running this closure or a function or a task
pub fn schedule(new: impl Into<Task>) {
    READY.push(new.into());
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

fn wait_next_task(mut should_abort: impl FnMut() -> bool) -> Option<Task> {
    loop {
        if let Some(task) = next_task() {
            return Some(task);
        }

        // debug!("no tasks, waiting for interrupts");
        wait();

        if should_abort() {
            return None;
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
