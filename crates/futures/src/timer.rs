use core::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{Future, FutureExt, Stream};
use hyperion_events::timer::SleepUntil;
use hyperion_instant::Instant;
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

#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct Sleep {
    inner: SleepUntil,
}

#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct Ticks {
    interval: Duration,
    next: SleepUntil,
}

//

impl Sleep {
    pub fn new(dur: Duration) -> Self {
        Self {
            inner: SleepUntil::new(Instant::now() + dur),
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
            self.next = sleep_until(self.next.deadline() + self.interval);
            Some(())
        })
    }
}

//
