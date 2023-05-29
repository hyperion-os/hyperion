use alloc::{
    collections::{BTreeMap, BinaryHeap},
    sync::Arc,
};
use core::{
    pin::Pin,
    task::{Context, Poll},
};

use chrono::Duration;
use futures_util::{task::AtomicWaker, Future};
use spin::{Lazy, Mutex, MutexGuard};

use crate::{
    driver::acpi::{
        apic::ApicId,
        hpet::{TimerN, HPET},
    },
    warn,
};

//

pub fn provide_sleep_wake() {
    let mut timer = DEADLINES.get(&ApicId::current()).unwrap().lock();
    if let Some(TimerWaker { deadline, waker }) = timer.pop() {
        assert!(HPET.main_counter_value() >= deadline);
        waker.wake();
    } else {
        warn!("Timer interrupt without active timers")
    }
}

pub const fn sleep(dur: Duration) -> Sleep {
    Sleep::new(dur)
}

//

#[must_use]
pub struct Sleep {
    dur: Duration,
    deadline: Option<u64>,

    // TODO: don't hold the timer lock
    timer: Option<MutexGuard<'static, TimerN>>,
}

//

impl Sleep {
    pub const fn new(dur: Duration) -> Self {
        Self {
            dur,
            deadline: None,
            timer: None,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let dur = self.dur;

        let deadline = *self.deadline.get_or_insert_with(|| {
            HPET.to_deadline(dur.num_nanoseconds().expect("Sleep is too long") as _)
        });

        if HPET.main_counter_value() >= deadline {
            return Poll::Ready(());
        }

        let timer = self.timer.get_or_insert_with(|| HPET.next_timer());
        let handler = timer.handler();

        let mut timer_priority_q = DEADLINES
            .get(&handler)
            .expect("TIMERS not initialized")
            .lock();

        let waker2 = Arc::new(AtomicWaker::new());
        let waker = waker2.clone();
        waker.register(cx.waker());
        timer_priority_q.push(TimerWaker { deadline, waker });

        timer.sleep_until(deadline);
        if HPET.main_counter_value() >= deadline {
            waker2.take();
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

//

static DEADLINES: Lazy<BTreeMap<ApicId, Mutex<BinaryHeap<TimerWaker>>>> = Lazy::new(|| {
    let mut res = BTreeMap::new();
    for apic in ApicId::iter() {
        res.insert(apic, <_>::default());
    }
    res
});

//

#[derive(Debug)]
struct TimerWaker {
    deadline: u64,
    waker: Arc<AtomicWaker>,
}

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
