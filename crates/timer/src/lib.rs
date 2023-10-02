#![no_std]

//

extern crate alloc;

use alloc::{collections::BinaryHeap, sync::Arc};

use crossbeam::atomic::AtomicCell;
use futures_util::task::AtomicWaker;
use hyperion_instant::Instant;
use hyperion_int_safe_lazy::IntSafeLazy;
use hyperion_log::warn;
use spin::Mutex;

//

// BinaryHeap::new isnt const? it only calls Vec::new internally which is const
pub static TIMER_DEADLINES: IntSafeLazy<Mutex<BinaryHeap<TimerWaker>>> =
    IntSafeLazy::new(|| Mutex::new(BinaryHeap::new()));

pub static TIMER_HANDLER: AtomicCell<fn()> = AtomicCell::new(provide_sleep_wake);

//

/// interrupt provided wakeup to a sleep
pub fn provide_sleep_wake() {
    let Some(deadlines) = TIMER_DEADLINES.get() else {
        return;
    };

    let mut timers = deadlines.lock();

    if let Some(TimerWaker { deadline, .. }) = timers.peek() {
        let now = Instant::now();
        if now < *deadline
        /* || (*deadline) + Duration::nanoseconds(10) > now */
        {
            return;
        }
    }

    if let Some(TimerWaker { waker, .. }) = timers.pop() {
        // assert!(now >= deadline, "{now} < {deadline}");
        waker.wake();
    } else {
        warn!("Timer interrupt without active timers")
    }
}

//

#[derive(Debug)]
pub struct TimerWaker {
    pub deadline: Instant,
    pub waker: Arc<AtomicWaker>,
}

//

impl PartialEq for TimerWaker {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}

impl Eq for TimerWaker {}

impl PartialOrd for TimerWaker {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerWaker {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        other.deadline.cmp(&self.deadline)
    }
}
