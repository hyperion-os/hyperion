use crate::{debug, error};
use x86_64::instructions::random::RdRand;

//

pub mod gdt;
pub mod idt;
pub mod pmm;
pub mod vmm;

//

pub fn early_boot_cpu() {
    gdt::init();
    idt::init();

    debug!("Re-enabling x86_64 interrupts");
    x86_64::instructions::interrupts::enable();
}

pub fn early_per_cpu() {}

pub fn rng_seed() -> u64 {
    RdRand::new().and_then(RdRand::get_u64).unwrap_or_else(|| {
        error!("Failed to generate a rng seed with x86_64 RDSEED");
        0
    })
}

pub fn done() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
