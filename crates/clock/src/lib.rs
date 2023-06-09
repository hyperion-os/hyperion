#![no_std]

//

use spin::Lazy;

//

pub trait ClockSource {
    fn tick_now(&self) -> u64;

    fn femtos_per_tick(&self) -> u64;
}

impl dyn ClockSource + Send + Sync {
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

//

pub static CLOCK_SOURCE: Lazy<&'static (dyn ClockSource + Send + Sync)> = Lazy::new(|| {
    // hyperion_pick_clock_source is safe to call if linked with hyperion-kernel
    // hyperion-kernel has `hyperion_pick_clock_source` correctly defined
    unsafe { hyperion_pick_clock_source() }
});

extern "Rust" {
    fn hyperion_pick_clock_source() -> &'static (dyn ClockSource + Send + Sync);
}
