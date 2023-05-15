use crate::{driver, error, smp::Cpu, warn};
use x86_64::instructions::{self as ins, interrupts as int, random::RdRand};

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

    /* driver::pic::PIT.lock().init(); */

    /*     driver::acpi::init(); */

    int::enable();
}

pub fn early_per_cpu(cpu: &Cpu) {
    int::disable();
    cpu::init(cpu);

    // driver::acpi::init();

    int::enable();

    /* if cfg!(debug_assertions) {
        warn!("[debug_assertions] {cpu} throwing a debug interrupt exception");
        debug_interrupt();
    } */
}

pub fn debug_interrupt() {
    int::int3();
}

pub fn rng_seed() -> u64 {
    RdRand::new().and_then(RdRand::get_u64).unwrap_or_else(|| {
        error!("Failed to generate a rng seed with x86_64 RDSEED");
        0
    })
}

pub fn wait_interrupt() {
    ins::hlt()
}

pub fn done() -> ! {
    loop {
        wait_interrupt()
    }
}
