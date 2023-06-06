use core::ops::{Add, Sub};

use chrono::Duration;

use super::checked::{CheckedAdd, CheckedSub};
use crate::driver::acpi::hpet::HPET;

//

// const NANOS_PER_FEMTOS: u64 = 1_000_000;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant {
    // clock tick period is based on the underlying clocksource
    // so this is not a nanosecond or picosecond or something
    inner: u64,
}

//

impl Instant {
    pub const ZERO: Self = Instant::new(0);

    pub fn now() -> Self {
        Self {
            inner: HPET.main_counter_value(),
        }
    }

    /// clock tick period is based on the underlying clocksource
    /// so this is not a nanosecond or picosecond or something
    pub const fn new(tick: u64) -> Self {
        Self { inner: tick }
    }

    pub const fn ticks(self) -> u64 {
        self.inner
    }

    pub fn elapsed(self) -> Duration {
        Self::now() - self
    }
}

impl CheckedAdd<Duration> for Instant {
    type Output = Self;

    fn checked_add(mut self, rhs: Duration) -> Option<Self::Output> {
        let nanos = rhs.num_nanoseconds()?;
        let ticks = HPET.nanos_to_ticks(nanos);
        self.inner = self.inner.checked_add_signed(ticks)?;

        Some(self)
    }
}

impl CheckedSub<Duration> for Instant {
    type Output = Self;

    fn checked_sub(mut self, rhs: Duration) -> Option<Self::Output> {
        let nanos = rhs.num_nanoseconds()?;
        let ticks = HPET.nanos_to_ticks(nanos);
        self.inner = self.inner.checked_add_signed(-ticks)?;

        Some(self)
    }
}

impl CheckedSub for Instant {
    type Output = Duration;

    fn checked_sub(self, rhs: Self) -> Option<Self::Output> {
        let ticks = (self.inner as i64).checked_sub(rhs.inner as i64)?;
        Some(Duration::nanoseconds(HPET.ticks_to_nanos(ticks)))
    }
}

impl Add<Duration> for Instant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        self.checked_add(rhs).unwrap()
    }
}

impl Sub<Duration> for Instant {
    type Output = Instant;

    fn sub(self, rhs: Duration) -> Self::Output {
        self.checked_sub(rhs).unwrap()
    }
}

impl Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub(rhs).unwrap()
    }
}
