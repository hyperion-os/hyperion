use alloc::collections::BinaryHeap;
use core::cmp::Reverse;

use hyperion_driver_acpi::hpet::HPET;
use hyperion_instant::Instant;
use spin::{Lazy, Mutex};

use crate::Task;

//

pub fn push(deadline: Instant, task: Task) {
    let mut sleep_q = SLEEP.lock();

    sleep_q.push(Reverse(SleepingTask { task, deadline }));

    HPET.next_timer()
        .sleep_until(HPET.nanos_to_ticks_u(deadline.nanosecond() as _));
}

/// # Warning
///
/// this iterator holds a lock
pub fn finished() -> impl Iterator<Item = Task> {
    let mut sleep_q = SLEEP.lock();
    let now = Instant::now();

    core::iter::from_fn(move || {
        if let Some(Reverse(SleepingTask { deadline, .. })) = sleep_q.peek() {
            if now < *deadline {
                return None;
            }
        }

        sleep_q.pop().map(|sleep| sleep.0.task)
    })
}

//

pub struct SleepingTask {
    task: Task,
    deadline: Instant,
}

impl PartialEq for SleepingTask {
    fn eq(&self, other: &Self) -> bool {
        self.deadline.eq(&other.deadline)
    }
}

impl Eq for SleepingTask {}

impl PartialOrd for SleepingTask {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SleepingTask {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.deadline.cmp(&other.deadline)
    }
}

//

static SLEEP: Lazy<Mutex<BinaryHeap<Reverse<SleepingTask>>>> =
    Lazy::new(|| Mutex::new(BinaryHeap::new()));
