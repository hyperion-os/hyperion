#![no_std]
#![feature(new_uninit, type_name_of_val, extract_if, sync_unsafe_cell, offset_of)]
#![allow(clippy::needless_return)]

//

use alloc::{
    borrow::Cow,
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use core::{
    convert::Infallible,
    mem::{offset_of, swap},
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering},
};

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
    task::{Ext, Pid, Process, Task, TaskState},
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

pub struct SchedulerStats {
    running: AtomicUsize,
    sleeping: AtomicUsize,
    ready: AtomicUsize,
    dropping: AtomicUsize,
}

/// `T` is extra process data
pub struct Scheduler<E: Ext> {
    ready: SegQueue<Task<E>>,
    running: AtomicBool,

    tls: Lazy<Tls<SchedulerTls<E>>>,

    // TODO: get rid of the slow dumbass spinlock mutexes everywhere
    processes: Mutex<BTreeMap<Pid, Weak<Process<E>>>>,

    stats: SchedulerStats,
}

impl<E: Ext> Scheduler<E> {
    pub const fn new() -> Self {
        Self {
            ready: SegQueue::new(),
            running: AtomicBool::new(false),

            tls: Lazy::new(|| {
                Tls::new(|| SchedulerTls {
                    active: Once::new(),
                    after: SegQueue::new(),
                    last_time: AtomicU64::new(0),
                    idle_time: AtomicU64::new(0),
                    initialized: AtomicBool::new(false),
                    idle: AtomicBool::new(false),

                    switch_last_active: AtomicPtr::new(ptr::null_mut()),
                })
            }),

            processes: Mutex::new(BTreeMap::new()),

            stats: SchedulerStats {
                running: AtomicUsize::new(0),
                sleeping: AtomicUsize::new(0),
                ready: AtomicUsize::new(0),
                dropping: AtomicUsize::new(0),
            },
        }
    }

    pub fn idle(&self) -> impl Iterator<Item = Duration> {
        Tls::inner(&self.tls).iter().map(|tls| {
            fn _assert_sync<T: Sync>(_: T) {}
            fn _assert<E: Ext>(tls: SchedulerTls<E>) {
                _assert_sync(tls.idle_time);
            }

            let tls = tls.get();
            // SAFETY: idle_time field is Sync
            let idle_time = unsafe {
                &*((tls as usize + offset_of!(SchedulerTls, idle_time)) as *const AtomicU64)
            };
            // let idle_time = &unsafe { &*tls }.idle_time;

            Duration::nanoseconds(idle_time.load(Ordering::Relaxed) as _)
        })
    }

    pub fn rename(&self, new_name: Cow<'static, str>) {
        *self.process().name.write() = new_name;
    }

    /// init this processors scheduling and
    /// immediately switch to the provided task
    pub fn init(&'static self, task: impl Into<Task<E>>) -> ! {
        hyperion_arch::int::disable();

        // init scheduler's custom page fault handler
        ints::PAGE_FAULT_HANDLER.store(page_fault::page_fault_handler);

        // init scheduler's custom general protection fault handler
        ints::GP_FAULT_HANDLER.store(|| {
            self.process()
                .should_terminate
                .store(true, Ordering::Relaxed);
            hyperion_log::debug!("GPF self term");
            self.stop();
        });

        // init HPET timer interrupts for sleep events
        timer::TIMER_HANDLER.store(|| sleep::wake_up_completed(None));

        // init periodic APIC timer interrutpts (optionally for RR-scheduling)
        apic::APIC_TIMER_HANDLER.store(|| {
            sleep::wake_up_completed(None);

            if !TLS.initialized.load(Ordering::SeqCst) {
                return;
            }

            if self.process().should_terminate.load(Ordering::Relaxed) {
                self.stop();
            }

            self.yield_now();
        });

        // hyperion_sync::init_futex(futex::wait, futex::wake);

        // init `Once` in TLS
        _ = self.task();
        let task = task.into();

        // mark scheduler as initialized and running
        if TLS.initialized.swap(true, Ordering::SeqCst) {
            panic!("should be called only once before any tasks are assigned to this processor")
        }
        self.running.store(true, Ordering::SeqCst);

        // switch to the init task
        switch_because(task, TaskState::Dropping, Cleanup::Drop);
        unreachable!("a destroyed thread cannot continue executing");
    }

    pub fn send(&self, target_pid: Pid, data: Cow<'static, [u8]>) -> Result<(), &'static str> {
        ipc::send(target_pid, data)
    }

    pub fn recv(&self) -> Cow<'static, [u8]> {
        ipc::recv()
    }

    pub fn recv_to(&self, buf: &mut [u8]) -> usize {
        ipc::recv_to(buf)
    }

    /// switch to another thread
    pub fn yield_now(&self) {
        self.update_cpu_usage();

        let Some(next) = self.next_task() else {
            // no tasks -> keep the current task running
            return;
        };
        switch_because(next, TaskState::Ready, Cleanup::Ready);
    }

    pub fn yield_now_wait(&self) {
        self.update_cpu_usage();

        let Some(next) = self.next_task() else {
            self.wait();
            // no tasks -> keep the current task running
            return;
        };
        switch_because(next, TaskState::Ready, Cleanup::Ready);
    }

    pub fn sleep(&self, duration: Duration) {
        self.sleep_until(Instant::now() + duration)
    }

    pub fn sleep_until(&self, deadline: Instant) {
        self.update_cpu_usage();

        let Ok(next) = self.wait_next_task(|| deadline.is_reached().then_some(())) else {
            return;
        };
        switch_because(next, TaskState::Sleeping, Cleanup::Sleep { deadline });
    }

    /// destroy the current thread
    /// and switch to another thread
    pub fn stop(&self) -> ! {
        self.update_cpu_usage();

        let next = self.wait_next_task::<Infallible>(|| None).unwrap();
        switch_because(next, TaskState::Dropping, Cleanup::Drop);

        unreachable!("a destroyed thread cannot continue executing");
    }

    /// spawn a new thread in the currently running process
    ///
    /// jumps into user space
    pub fn spawn_userspace(&self, fn_ptr: u64, fn_arg: u64) {
        self.spawn(move || {
            let stack_top = self.task().user_stack.lock().top;

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
    pub fn schedule(&self, new: impl Into<Task<E>>) -> Pid {
        let task = new.into();
        let pid = task.pid;
        self.ready.push(task);
        pid
    }

    pub fn spawn(&self, new: impl FnOnce() + Send + 'static) {
        self.ready.push(Task::thread(self.task(), new));
    }

    fn swap_current(&self, mut new: Task<E>) -> Task<E> {
        swap(&mut new, &mut self.get_task().lock());
        new
    }

    #[must_use]
    fn cpu_time_elapsed(&self) -> u64 {
        let now = HPET.nanos() as u64;
        let last = self.last_time().swap(now, Ordering::SeqCst);

        now - last
    }

    fn reset_cpu_timer(&self) {
        _ = self.cpu_time_elapsed();
    }

    /// increase the task info's cpu usage field
    fn update_cpu_usage(&self) {
        let elapsed = self.cpu_time_elapsed();

        self.task().nanos.fetch_add(elapsed, Ordering::Relaxed);
    }

    fn update_cpu_idle(&self) {
        let elapsed = self.cpu_time_elapsed();

        self.idle_time().fetch_add(elapsed, Ordering::Relaxed);
    }

    fn wait_next_task<Err>(
        &self,
        mut should_abort: impl FnMut() -> Option<Err>,
    ) -> Result<Task<E>, Err> {
        loop {
            if let Some(task) = self.next_task() {
                return Ok(task);
            }

            // debug!("no tasks, waiting for interrupts");
            self.wait();

            if let Some(err) = should_abort() {
                return Err(err);
            }
        }
    }

    fn next_task(&self) -> Option<Task<E>> {
        self.ready.pop()
    }

    fn wait(&self) {
        self.reset_cpu_timer();
        self.tls.idle.store(true, Ordering::SeqCst);
        int::wait();
        self.tls.idle.store(false, Ordering::SeqCst);
        self.update_cpu_idle();
    }

    fn cleanup(&self) {
        let after = self.after();

        while let Some(next) = after.pop() {
            next.run();
        }
    }

    pub fn task(&self) -> Task<E> {
        (*self.get_task().lock()).clone()
    }

    pub fn process(&self) -> Arc<Process<E>> {
        self.get_task().lock().process.clone()
    }

    pub fn running(&self) -> bool {
        // short circuits and doesnt init TLS unless it has to
        self.running.load(Ordering::SeqCst) && self.tls.initialized.load(Ordering::SeqCst)
    }

    fn get_task(&'static self) -> &'static Mutex<Task<E>> {
        self.tls.active.call_once(|| Mutex::new(Task::bootloader()))
    }

    fn after(&'static self) -> &'static SegQueue<CleanupTask> {
        &self.tls.after
    }

    fn last_time(&'static self) -> &'static AtomicU64 {
        &self.tls.last_time
    }

    fn idle_time(&'static self) -> &'static AtomicU64 {
        &self.tls.idle_time
    }
}

//

struct SchedulerTls<E> {
    active: Once<Mutex<Task<E>>>,
    after: SegQueue<CleanupTask>,
    last_time: AtomicU64,
    idle_time: AtomicU64,
    initialized: AtomicBool,
    idle: AtomicBool,

    switch_last_active: AtomicPtr<TaskInner<E>>,
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
