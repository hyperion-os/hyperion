#![no_std]
#![feature(abi_x86_interrupt)]

//

extern crate alloc;

use hyperion_boot_interface::Cpu;
use hyperion_log::{debug, error};
use spin::{Barrier, Once};
use x86_64::instructions::random::RdRand;

//

pub mod cpu;
pub mod pmm;
pub mod vmm;

//

pub fn early_boot_cpu() {
    int::disable();

    cpu::init(&hyperion_boot::boot_cpu());

    // TODO: enable PICS

    int::enable();
}

/// every LAPIC is initialized after any CPU can exit this function call
pub fn early_per_cpu(cpu: &Cpu) {
    int::disable();

    let cpus = hyperion_boot::cpu_count();

    macro_rules! barrier {
        ($print:expr, $name:ident) => {
            if $print {
                debug!("waiting: {}", stringify!($name));
            }
            static $name: Once<Barrier> = Once::new();
            $name.call_once(|| Barrier::new(cpus)).wait();
            if $print {
                debug!("done waiting: {}", stringify!($name));
            }
        };
    }

    barrier!(cpu.is_boot(), PRE_APIC);

    cpu::init(cpu);

    hyperion_drivers::acpi::init();

    barrier!(cpu.is_boot(), POST_APIC);

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

pub fn spin_loop() {
    core::hint::spin_loop()
}

pub fn done() -> ! {
    loop {
        // spin_loop();
        int::wait()
    }
}
