#![no_std]

//

extern crate alloc;

use crossbeam::atomic::AtomicCell;
use spin::Once;

//

pub fn get() -> &'static dyn ClockSource {
    let clock = CLOCK_SOURCE
        .try_call_once(|| PICK_CLOCK_SOURCE.load()().ok_or(()))
        .ok()
        .copied();

    clock.unwrap_or(&NopClock)
}

pub fn set_source_picker(f: fn() -> Option<&'static dyn ClockSource>) {
    PICK_CLOCK_SOURCE.store(f);
}

//

pub trait ClockSource: Send + Sync {
    fn nanosecond_now(&self) -> u128;

    fn trigger_interrupt_at(&self, nanosecond: u128);

    fn _apic_sleep_simple_blocking(&self, micros: u16, pre: &mut dyn FnMut());
}

//

pub struct NopClock;

//

impl ClockSource for NopClock {
    fn nanosecond_now(&self) -> u128 {
        0
    }

    fn trigger_interrupt_at(&self, _: u128) {}

    fn _apic_sleep_simple_blocking(&self, _: u16, pre: &mut dyn FnMut()) {
        pre();
    }
}

//

static CLOCK_SOURCE: Once<&'static dyn ClockSource> = Once::new();

static PICK_CLOCK_SOURCE: AtomicCell<fn() -> Option<&'static dyn ClockSource>> =
    AtomicCell::new(|| None);
