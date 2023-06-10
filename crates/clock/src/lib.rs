#![no_std]

//

extern crate alloc;

use alloc::boxed::Box;

use spin::{Lazy, Mutex};

//

pub static CLOCK_SOURCE: Lazy<&'static dyn ClockSource> = Lazy::new(|| {
    let picker = PICK_CLOCK_SOURCE.lock();
    picker().unwrap_or(&NopClock)
});

pub static PICK_CLOCK_SOURCE: Mutex<fn() -> Option<&'static dyn ClockSource>> = Mutex::new(|| None);

//

pub trait ClockSource: Send + Sync {
    fn tick_now(&self) -> u64;

    fn femtos_per_tick(&self) -> u64;

    fn trigger_interrupt_at(&self, deadline: u64);

    fn _apic_sleep_simple_blocking(&self, micros: u16, pre: &mut dyn FnMut());
}

impl dyn ClockSource {
    /// `nanos` is nanos from now
    pub fn nanos_to_deadline(&self, nanos: u64) -> u64 {
        self.tick_now() + self.nanos_to_ticks_u(nanos)
    }

    pub fn nanos_to_ticks_u(&self, nanos: u64) -> u64 {
        (nanos as u128 * 1_000_000 / self.femtos_per_tick() as u128) as u64
    }

    pub fn nanos_to_ticks_i(&self, nanos: i64) -> i64 {
        (nanos as i128 * 1_000_000 / self.femtos_per_tick() as i128) as i64
    }

    pub fn ticks_to_nanos_u(&self, ticks: u64) -> u64 {
        (ticks as u128 * self.femtos_per_tick() as u128 / 1_000_000) as u64
    }

    pub fn ticks_to_nanos_i(&self, ticks: i64) -> i64 {
        (ticks as i128 * self.femtos_per_tick() as i128 / 1_000_000) as i64
    }
}

pub struct NopClock;

impl ClockSource for NopClock {
    fn tick_now(&self) -> u64 {
        0
    }

    fn femtos_per_tick(&self) -> u64 {
        u64::MAX
    }

    fn trigger_interrupt_at(&self, _: u64) {}

    fn _apic_sleep_simple_blocking(&self, _: u16, pre: &mut dyn FnMut()) {
        pre();
    }
}
