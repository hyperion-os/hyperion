use core::hash::{Hash, Hasher, SipHasher};

use crate::{error, smp::Cpu};
use alloc::format;
use x86_64::{instructions::random::RdRand, structures::idt::InterruptDescriptorTable};

//

pub mod cpu;
pub mod pmm;
pub mod vmm;

mod gdt;
mod idt;

//

pub fn early_boot_cpu() {
    // x86_64::instructions::interrupts::disable();
    // gdt::init();
    // idt::init();
    // x86_64::instructions::interrupts::enable();

    // x86_64::instructions::interrupts::int3();

    x86_64::instructions::interrupts::disable();
    // cpu::init has a guard to init GDT&IDT only once for the boot cpu
    cpu::init(&Cpu::new_boot());
    let idt_data: &InterruptDescriptorTable =
        unsafe { &*x86_64::instructions::tables::sidt().base.as_ptr() };
    let mut hasher = SipHasher::new_with_keys(543789, 54780);
    format!("{idt_data:?}").hash(&mut hasher);
    error!("{}", hasher.finish());
    x86_64::instructions::interrupts::disable();
    idt::init();
    x86_64::instructions::interrupts::enable();
    let idt_data: &InterruptDescriptorTable =
        unsafe { &*x86_64::instructions::tables::sidt().base.as_ptr() };
    let mut hasher = SipHasher::new_with_keys(543789, 54780);
    format!("{idt_data:?}").hash(&mut hasher);
    error!("{}", hasher.finish());
    x86_64::instructions::interrupts::enable();

    x86_64::instructions::interrupts::int3();
}

pub fn early_per_cpu(cpu: &Cpu) {
    // x86_64::instructions::interrupts::disable();
    // cpu::init(cpu);
    // x86_64::instructions::interrupts::enable();
}

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
