#![no_std]
#![feature(new_uninit, type_name_of_val, extract_if)]
#![allow(clippy::needless_return)]

//

use alloc::{
    borrow::Cow,
    collections::BTreeMap,
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};
use core::{
    mem::swap,
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
};

use crossbeam_queue::SegQueue;
use hyperion_arch::{context::switch as ctx_switch, cpu::ints, int, tls::Tls};
use hyperion_driver_acpi::hpet::HPET;
use hyperion_instant::Instant;
use hyperion_log::*;
use hyperion_timer::TIMER_HANDLER;
use spin::{Lazy, Mutex, MutexGuard, Once};
use time::Duration;
use x86_64::VirtAddr;

use self::{
    cleanup::{Cleanup, CleanupTask},
    task::{Pid, Task, TaskInfo, TaskMemory, TaskState},
};
use crate::task::{switch_because, TaskInner, TaskThread};

//

extern crate alloc;

pub mod cleanup;
pub mod process;
pub mod sleep;
pub mod task;

mod page_fault;

//

pub static READY: SegQueue<Task> = SegQueue::new();
pub static RUNNING: AtomicBool = AtomicBool::new(false);

/// task info
pub static TASKS: Mutex<Vec<Weak<TaskInfo>>> = Mutex::new(vec![]);

// TODO: concurrent map
pub static TASK_MEM: Mutex<BTreeMap<Pid, Weak<TaskMemory>>> = Mutex::new(BTreeMap::new());

/* pub trait LockNamed<T: ?Sized> {
    fn lock_named(&self, name: &'static str) -> NamedMutexGuard<T>;
}

impl<T: ?Sized> LockNamed<T> for Mutex<T> {
    fn lock_named(&self, name: &'static str) -> NamedMutexGuard<T> {
        debug!("locking {name}");
        NamedMutexGuard {
            inner: self.lock(),
            name,
        }
    }
}

pub struct NamedMutexGuard<'a, T: ?Sized + 'a> {
    inner: MutexGuard<'a, T>,
    name: &'static str,
}

impl<'a, T: ?Sized> core::ops::Deref for NamedMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T: ?Sized> core::ops::DerefMut for NamedMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'a, T: ?Sized> Drop for NamedMutexGuard<'a, T> {
    fn drop(&mut self) {
        debug!("unlocking {}", self.name);
    }
} */

//

pub fn send(target_pid: Pid, data: Cow<'static, [u8]>) -> Result<(), &'static str> {
    let mem = TASK_MEM
        .lock()
        .get(&target_pid)
        .and_then(|mem_weak_ref| mem_weak_ref.upgrade())
        .ok_or("no such process")?;

    mem.simple_ipc.lock().push(data);
    let recv_task = mem.simple_ipc_waiting.lock().take();

    if let Some(recv_task) = recv_task {
        // READY.push(recv_task);
        switch_because(recv_task, TaskState::Ready, Cleanup::Ready);
    }

    Ok(())
}

pub fn recv() -> Cow<'static, [u8]> {
    let mem: Arc<TaskMemory> = {
        let active = lock_active();
        (*active.memory).clone()
    };

    if let Some(data) = mem.simple_ipc.lock().pop() {
        return data;
    }

    let mut data = None; // data while waiting for the next task
    let Some(next) = wait_next_task(|| {
        data = mem.simple_ipc.lock().pop();
        data.is_some()
    }) else {
        return data.unwrap();
    };
    switch_because(next, TaskState::Sleeping, Cleanup::SimpleIpcWait);

    // data after a signal of receiving data
    lock_active().memory.simple_ipc.lock().pop().unwrap()
}

/* pub fn task_memory() -> Arc<TaskMemory> {
    let current: *const Arc<TaskMemory> = CURRENT_ADDRESS_SPACE.as_ptr();
    let current = unsafe { (*current).clone() };
    assert_eq!(current.dbg_magic_byte, *MAGIC_DEBUG_BYTE);
    current
} */

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

pub fn rename(new_name: Cow<'static, str>) {
    *lock_active().info().name.write() = new_name;
}

/// init this processors scheduling
pub fn init() -> ! {
    hyperion_arch::int::disable();

    ints::PAGE_FAULT_HANDLER.store(page_fault::page_fault_handler);
    TIMER_HANDLER.store(|| {
        for task in sleep::finished() {
            READY.push(task);
        }
    });
    hyperion_driver_acpi::apic::APIC_TIMER_HANDLER.store(|| {
        for task in sleep::finished() {
            // warn!("TODO: fix APIC timer waking up HPET timers");
            READY.push(task);
        }

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

    _ = get_active();
    if TLS.initialized.swap(true, Ordering::SeqCst) {
        panic!("should be called only once before any tasks are assigned to this processor")
    }
    RUNNING.store(true, Ordering::SeqCst);

    stop();
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
    let thread = Task::new(TaskInner::thread(lock_active(), move || {
        let stack_top = { lock_active().thread.user_stack.lock().top };

        unsafe {
            hyperion_arch::syscall::userland(
                VirtAddr::new(fn_ptr),
                stack_top,
                stack_top.as_u64(),
                fn_arg,
            )
        };
    }));
    READY.push(thread);

    debug!("spawning a pthread");
}
/// spawn a new process running this closure or a function or a task
pub fn schedule(new: impl Into<Task>) {
    READY.push(new.into());
}

fn swap_current(mut new: Task) -> Task {
    swap(&mut new, &mut lock_active());
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

    lock_active()
        .info()
        .nanos
        .fetch_add(elapsed, Ordering::Relaxed);
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

    switch_last_active: AtomicPtr<TaskThread>,
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

pub fn lock_active() -> MutexGuard<'static, Task> {
    get_active().lock()
}

pub fn try_lock_active() -> Option<MutexGuard<'static, Task>> {
    get_active().try_lock()
}

pub fn running() -> bool {
    // short circuits and doesnt init TLS unless it has to
    RUNNING.load(Ordering::SeqCst) && TLS.initialized.load(Ordering::SeqCst)
}

fn get_active() -> &'static Mutex<Task> {
    TLS.active
        .call_once(|| Mutex::new(Task::new(TaskInner::bootloader())))
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

extern "sysv64" fn thread_entry() -> ! {
    cleanup();
    let job = lock_active().take_job().expect("no active jobs");
    job();
    stop();
}
