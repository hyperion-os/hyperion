#![no_std]

//

use core::ops::{Add, Sub};

use time::Duration;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant {
    nanosecond: u128,
}

//

impl Instant {
    pub const ZERO: Self = Instant::new(0);

    #[must_use]
    pub fn now() -> Self {
        Self {
            nanosecond: hyperion_clock::get().nanosecond_now(),
        }
    }

    #[must_use]
    pub const fn new(nanosecond: u128) -> Self {
        Self { nanosecond }
    }

    #[must_use]
    pub const fn nanosecond(self) -> u128 {
        self.nanosecond
    }

    #[must_use]
    pub fn elapsed(self) -> Duration {
        Self::now() - self
    }

    #[must_use]
    pub fn is_reached(self) -> bool {
        self < Self::now()
    }
}

//

pub trait CheckedAdd<Rhs = Self> {
    type Output;

    fn checked_add(self, rhs: Rhs) -> Option<Self::Output>;
}

pub trait CheckedSub<Rhs = Self> {
    type Output;

    fn checked_sub(self, rhs: Rhs) -> Option<Self::Output>;
}

impl CheckedAdd<Duration> for Instant {
    type Output = Self;

    fn checked_add(mut self, rhs: Duration) -> Option<Self::Output> {
        self.nanosecond = self
            .nanosecond
            .saturating_add_signed(rhs.whole_nanoseconds());
        Some(self)
    }
}

impl CheckedSub<Duration> for Instant {
    type Output = Self;

    fn checked_sub(mut self, rhs: Duration) -> Option<Self::Output> {
        self.nanosecond = self
            .nanosecond
            .saturating_add_signed(-rhs.whole_nanoseconds());
        Some(self)
    }
}

impl CheckedSub for Instant {
    type Output = Duration;

    fn checked_sub(self, rhs: Self) -> Option<Self::Output> {
        let lhs: i128 = self.nanosecond.try_into().ok()?;
        let rhs: i128 = rhs.nanosecond.try_into().ok()?;

        let nanos = lhs.checked_sub(rhs)?;

        let seconds = (nanos / 1_000_000_000) as i64;
        let nanos = (nanos % 1_000_000_000) as i64;

        Some(Duration::seconds(seconds) + Duration::nanoseconds(nanos))
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
