use alloc::collections::{BTreeMap, VecDeque};
use core::{
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperion_sync::spinlock;

use crate::{
    cleanup::Cleanup,
    task::{switch_because, Task, TaskState},
    wait_next_task, READY,
};

//

static WAITING: spinlock::Mutex<BTreeMap<usize, VecDeque<Task>>> =
    spinlock::Mutex::new(BTreeMap::new());

//

/// if the value at `addr` is eq `val`, go to sleep
pub fn wait(addr: NonNull<AtomicUsize>, val: usize) {
    struct IsNeq;

    let var: &'static AtomicUsize = unsafe { &*addr.as_ptr() };

    if var.load(Ordering::SeqCst) != val {
        return;
    }

    let next = wait_next_task(|| {
        if var.load(Ordering::SeqCst) != val {
            Some(IsNeq)
        } else {
            None
        }
    });

    match next {
        Ok(next) => switch_because(next, TaskState::Sleeping, Cleanup::Wait { addr, val }),
        Err(IsNeq) => return,
    }
}

/// wake up threads waiting for events on this `addr`
pub fn wake(addr: NonNull<AtomicUsize>, num: usize) {
    let mut waiting = WAITING.lock();

    if let Some(waiting_on_addr) = waiting.get_mut(&(addr.as_ptr() as usize)) {
        // if drain on VecDeque front is optimized:
        waiting_on_addr.drain(..num.min(waiting_on_addr.len()));

        // for _ in 0..num {
        //     if waiting_on_addr.pop_front().is_none() {
        //         break;
        //     }
        // }
    }
}

/// post switch cleanup
pub fn cleanup(addr: NonNull<AtomicUsize>, val: usize, task: Task) {
    let var: &'static AtomicUsize = unsafe { &*addr.as_ptr() };

    let mut waiting = WAITING.lock();

    if var.load(Ordering::SeqCst) != val {
        READY.push(task);
        return;
    }

    waiting
        .entry(var.as_ptr() as usize)
        .or_default()
        .push_back(task);

    drop(waiting);
}
