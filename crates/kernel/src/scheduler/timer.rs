use alloc::{
    collections::{BTreeMap, BinaryHeap},
    sync::Arc,
};
use core::{
    pin::Pin,
    task::{Context, Poll},
};

use chrono::Duration;
use futures_util::{task::AtomicWaker, Future, FutureExt, Stream};
use hyperion_instant::Instant;
use hyperion_int_safe_lazy::IntSafeLazy;
use hyperion_log::warn;
use spin::{Lazy, Mutex};

use crate::driver::acpi::{apic::ApicId, hpet::HPET};

//

/// interrupt provided wakeup to a sleep
pub fn provide_sleep_wake() {
    let Some(deadlines) = DEADLINES.get() else {
        return
    };

    let mut timers = deadlines.lock();

    if let Some(TimerWaker { deadline, .. }) = timers.peek() {
        if Instant::now() < *deadline {
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

/// async sleep until deadline
pub const fn sleep_until(deadline: Instant) -> SleepUntil {
    SleepUntil::new(deadline)
}

/// async sleep duration
pub fn sleep(dur: Duration) -> Sleep {
    Sleep::new(dur)
}

/// async sleep repeat
///
/// does not get desynced from the previous ticks
pub fn ticks(interval: Duration) -> Ticks {
    Ticks {
        interval,
        next: sleep_until(Instant::now() + interval),
    }
}

//

#[must_use]
pub struct SleepUntil {
    deadline: Instant,
    sleeping: bool,
}

#[must_use]
pub struct Sleep {
    inner: SleepUntil,
}

#[must_use]
pub struct Ticks {
    interval: Duration,
    next: SleepUntil,
}

//

impl SleepUntil {
    pub const fn new(deadline: Instant) -> Self {
        Self {
            deadline,
            sleeping: false,
        }
    }
}

impl Sleep {
    pub fn new(dur: Duration) -> Self {
        Self {
            inner: SleepUntil::new(Instant::now() + dur),
        }
    }
}

impl Future for SleepUntil {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let deadline = self.deadline;

        if Instant::now() >= deadline {
            return Poll::Ready(());
        }

        if self.sleeping {
            return Poll::Pending;
        }
        self.sleeping = true;

        let mut timer = HPET.next_timer();

        // insert the new deadline before invoking sleep,
        // so that the waker is there before the interrupt happens
        let waker = Arc::new(AtomicWaker::new());
        let waker2 = waker.clone();
        waker.register(cx.waker());
        DEADLINES
            .get_force()
            .lock()
            .push(TimerWaker { deadline, waker });

        timer.sleep_until(deadline.ticks());

        if Instant::now() >= deadline {
            waker2.take();
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.inner.poll_unpin(cx)
    }
}

impl Stream for Ticks {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.next.poll_unpin(cx).map(|_| {
            self.next = sleep_until(self.next.deadline + self.interval);
            Some(())
        })
    }
}

//

// BinaryHeap::new isnt const? it only calls Vec::new internally which is const
static DEADLINES: IntSafeLazy<Mutex<BinaryHeap<TimerWaker>>> =
    IntSafeLazy::new(|| Mutex::new(BinaryHeap::new()));

//

#[derive(Debug)]
struct TimerWaker {
    deadline: Instant,
    waker: Arc<AtomicWaker>,
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
        other.deadline.partial_cmp(&self.deadline)
    }
}

impl Ord for TimerWaker {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        other.deadline.cmp(&self.deadline)
    }
}
