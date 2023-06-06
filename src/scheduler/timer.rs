use alloc::collections::{BTreeMap, BinaryHeap};
use core::{
    pin::Pin,
    task::{Context, Poll},
};

use chrono::Duration;
use futures_util::{task::AtomicWaker, Future};
use spin::{Lazy, Mutex, MutexGuard};

use crate::{
    arch::int,
    driver::acpi::{
        apic::ApicId,
        hpet::{TimerN, TimerNHandle, HPET},
    },
    util::int_safe_lazy::IntSafeLazy,
};

//

pub fn provide_sleep_wake() {
    let Some(deadlines) = DEADLINES.get() else {
        return
    };

    let now = HPET.main_counter_value();
    let mut timers = deadlines.get(&ApicId::current()).unwrap().lock();

    if let Some(TimerWaker { deadline, .. }) = timers.peek() {
        if now < *deadline {
            return;
        }
    }

    if let Some(TimerWaker { deadline, waker }) = timers.pop() {
        // assert!(now >= deadline, "{now} < {deadline}");
        // crate::debug!("wakeup call {deadline} {now}");
        waker.wake();
        // crate::debug!("wakeup call done");
    } else {
        crate::warn!("Timer interrupt without active timers")
    }
}

pub const fn sleep(dur: Duration) -> Sleep {
    Sleep::new(dur)
}

//

#[must_use]
pub struct Sleep {
    dur: Duration,
    inner: Option<SleepLazy>,
}

/* #[must_use]
pub struct SleepUntil {} */

//

impl Sleep {
    pub const fn new(dur: Duration) -> Self {
        Self { dur, inner: None }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let deadlines = DEADLINES.get_force();

        let dur = self.dur;
        let mut timer = None;
        let SleepLazy { deadline, handler } = *self.inner.get_or_insert_with(|| {
            let (inner, _timer) = SleepLazy::new(dur);
            timer = Some(_timer);
            inner
        });

        // crate::debug!("poll deadline test");
        if HPET.main_counter_value() >= deadline {
            // crate::debug!("poll stops with ready");
            return Poll::Ready(());
        }

        // insert the new deadline before invoking sleep,
        // so that the waker is there before the interrupt happens
        let waker = AtomicWaker::new();
        waker.register(cx.waker());
        {
            deadlines
                .get(&handler)
                .expect("TIMERS not initialized")
                .lock()
                .push(TimerWaker { deadline, waker });
        }

        if let Some(mut timer) = timer {
            timer.sleep_until(deadline);
        }

        // crate::debug!("poll stops with pending");
        Poll::Pending
        /* if HPET.main_counter_value() >= deadline {
            waker2.take();
            Poll::Ready(())
        } else {
            Poll::Pending
        } */
    }
}

//

static DEADLINES: IntSafeLazy<BTreeMap<ApicId, Mutex<BinaryHeap<TimerWaker>>>> =
    IntSafeLazy::new(|| ApicId::iter().map(|apic| (apic, <_>::default())).collect());

//

#[derive(Debug)]
struct TimerWaker {
    deadline: u64,
    waker: AtomicWaker,
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

//

#[derive(Debug, Clone, Copy)]
struct SleepLazy {
    deadline: u64,
    handler: ApicId,
}

//

impl SleepLazy {
    fn new(dur: Duration) -> (Self, TimerNHandle) {
        let hpet = Lazy::force(&HPET);

        let timer = hpet.next_timer();
        let deadline = hpet.to_deadline(dur.num_nanoseconds().expect("Sleep is too long") as _);
        let handler = timer.handler();

        (Self { deadline, handler }, timer)
    }
}
