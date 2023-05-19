use crate::{driver, error, smp::Cpu, warn};
use x86_64::instructions::{self as ins, random::RdRand};

//

pub mod cpu;
// pub mod mem;
pub mod pmm;
pub mod vmm;

//

pub fn early_boot_cpu() {
    int::disable();

    cpu::init(&Cpu::new_boot());

    {
        let pics = &*driver::pic::PICS;
        driver::rtc::RTC.now();

        pics.lock().enable();
    }

    driver::acpi::init();

    int::enable();
}

pub fn early_per_cpu(cpu: &Cpu) {
    int::disable();
    cpu::init(cpu);

    // driver::acpi::init();

    int::enable();

    /* if cfg!(debug_assertions) {
        warn!("[debug_assertions] {cpu} throwing a debug interrupt exception");
        int::debug();
    } */
}

pub fn rng_seed() -> u64 {
    RdRand::new().and_then(RdRand::get_u64).unwrap_or_else(|| {
        error!("Failed to generate a rng seed with x86_64 RDSEED");
        0
    })
}

pub mod int {
    use x86_64::instructions::interrupts as int;

    pub fn debug() {
        int::int3();
    }

    pub fn disable() {
        int::disable()
    }

    pub fn enable() {
        int::enable()
    }

    pub fn are_enabled() -> bool {
        int::are_enabled()
    }

    pub fn without<T>(f: impl FnOnce() -> T) -> T {
        int::without_interrupts(f)
    }

    pub fn wait() {
        int::enable_and_hlt()
    }
}

pub fn done() -> ! {
    loop {
        int::wait()
    }
}
