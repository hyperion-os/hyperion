use alloc::collections::BinaryHeap;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use hyperion_instant::Instant;
use spin::Mutex;

//

/// interrupt provided wakeup to a sleep
pub fn wake() {
    let mut timers = TIMER_DEADLINES.lock();

    let Some(TimerWaker { deadline, .. }) = timers.peek() else {
        // hyperion_log::debug!("incorrect timer wake");
        return;
    };

    if !deadline.is_reached() {
        // hyperion_log::debug!("incorrect timer wake");
        return;
    }

    let TimerWaker { waker, .. } = timers.pop().unwrap();
    waker.wake();
}

pub const fn sleep_until(deadline: Instant) -> SleepUntil {
    SleepUntil::new(deadline)
}

//

#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct SleepUntil {
    deadline: Instant,
    sleeping: bool,
}

impl SleepUntil {
    pub const fn new(deadline: Instant) -> Self {
        Self {
            deadline,
            sleeping: false,
        }
    }

    pub const fn deadline(self) -> Instant {
        self.deadline
    }
}

impl Future for SleepUntil {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let deadline = self.deadline;

        if !self.sleeping {
            self.get_mut().sleeping = true;
            if deadline.is_reached() {
                return Poll::Ready(());
            }

            hyperion_clock::get().trigger_interrupt_at(deadline.nanosecond());
            let waker = cx.waker().clone();
            TIMER_DEADLINES.lock().push(TimerWaker { deadline, waker });
        }

        if deadline.is_reached() {
            return Poll::Ready(());
        }

        Poll::Pending
    }
}

//

#[derive(Debug)]
struct TimerWaker {
    pub deadline: Instant,
    pub waker: Waker,
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

// BinaryHeap::new isnt const? it only calls Vec::new internally which is const
static TIMER_DEADLINES: Mutex<BinaryHeap<TimerWaker>> = Mutex::new(BinaryHeap::new());
