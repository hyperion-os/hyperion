use alloc::collections::{BTreeMap, VecDeque};
use core::{
    mem::ManuallyDrop,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperion_mem::{to_higher_half, vmm::PageMapImpl};
use x86_64::{PhysAddr, VirtAddr};

use crate::{
    cleanup::Cleanup,
    process,
    task::{switch_because, Task, TaskState},
    wait_next_task, READY,
};

//

/// if the value at `addr` is eq `val`, go to sleep
pub fn wait(addr: &AtomicUsize, val: usize) {
    if addr.load(Ordering::SeqCst) != val {
        return;
    }

    let next = wait_next_task(|| should_cancel(addr, val).then_some(()));

    if let Ok(next) = next {
        let addr: NonNull<AtomicUsize> = addr.into();

        let addr = process()
            .address_space
            .page_map
            .virt_to_phys(VirtAddr::from_ptr(addr.as_ptr()))
            .unwrap();

        switch_because(next, TaskState::Sleeping, Cleanup::Wait { addr, val })
    }
}

/// wake up threads waiting for events on this `addr`
pub fn wake(addr: &AtomicUsize, num: usize) {
    let addr = process()
        .address_space
        .page_map
        .virt_to_phys(VirtAddr::from_ptr(addr.as_ptr()))
        .unwrap();

    WAITING.pop(addr.as_u64() as usize, num);
}

/// post switch cleanup
pub fn cleanup(addr: PhysAddr, val: usize, task: Task) {
    let var = unsafe { &*to_higher_half(addr).as_ptr::<AtomicUsize>() };

    let cancel = WAITING.push(addr.as_u64() as usize, task, || {
        // cancel the wait if var == val
        should_cancel(var, val)
    });

    if let Some(task) = cancel {
        READY.push(task);
    }
}

fn should_cancel(var: &AtomicUsize, val: usize) -> bool {
    var.load(Ordering::SeqCst) != val
}

//

static WAITING: Waiters = Waiters::new();

//

struct Waiters {
    addrs: spin::Mutex<BTreeMap<usize, VecDeque<Waiter>>>,
}

impl Waiters {
    pub const fn new() -> Self {
        Self {
            addrs: spin::Mutex::new(BTreeMap::new()),
        }
    }

    pub fn push(&self, addr: usize, task: Task, unless: impl FnOnce() -> bool) -> Option<Task> {
        let mut addrs = self.addrs.lock();

        if unless() {
            return Some(task);
        }

        addrs.entry(addr).or_default().push_back(Waiter::new(task));

        None
    }

    pub fn pop(&self, addr: usize, count: usize) {
        let mut addrs = self.addrs.lock();

        if let Some(waiting_on_addr) = addrs.get_mut(&addr) {
            let new_len = waiting_on_addr.len().saturating_sub(count);
            waiting_on_addr.truncate(new_len);
        }
    }
}

//

struct Waiter {
    task: ManuallyDrop<Task>,
}

impl Waiter {
    pub const fn new(task: Task) -> Self {
        Self {
            task: ManuallyDrop::new(task),
        }
    }
}

impl Drop for Waiter {
    fn drop(&mut self) {
        READY.push(unsafe { ManuallyDrop::take(&mut self.task) })
    }
}
