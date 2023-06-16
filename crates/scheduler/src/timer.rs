use alloc::sync::Arc;
use core::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{task::AtomicWaker, Future, FutureExt, Stream};
use hyperion_clock::CLOCK_SOURCE;
use hyperion_instant::Instant;
use hyperion_timer::{TimerWaker, TIMER_DEADLINES};
use time::Duration;

//

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

        // insert the new deadline before invoking sleep,
        // so that the waker is there before the interrupt happens
        let waker = Arc::new(AtomicWaker::new());
        let waker2 = waker.clone();
        waker.register(cx.waker());
        TIMER_DEADLINES
            .get_force()
            .lock()
            .push(TimerWaker { deadline, waker });

        CLOCK_SOURCE.trigger_interrupt_at(deadline.nanosecond());

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
