#![no_std]
#![feature(
    abi_x86_interrupt,
    custom_test_frameworks,
    naked_functions,
    new_uninit,
    asm_const,
    const_refs_to_cell
)]

//

extern crate alloc;

use core::sync::atomic::{AtomicUsize, Ordering};

use hyperion_boot_interface::Cpu;
use hyperion_log::error;
use x86_64::{instructions::random::RdRand, registers::model_specific::Msr};

//

pub mod context;
pub mod cpu;
pub mod paging;
pub mod pmm;
pub mod stack;
pub mod syscall;
pub mod tls;
pub mod vmm;

//

pub const IA32_TSC_AUX: u32 = 0xC0000103;

//

pub fn early_boot_cpu() {
    int::disable();

    cpu::init(&hyperion_boot::boot_cpu());

    // int::enable();
}

/// every LAPIC is initialized after any CPU can exit this function call
///
/// [`early_boot_cpu`] should have been called already
pub fn early_per_cpu(cpu: &Cpu) {
    static CPU_ID_GEN: AtomicUsize = AtomicUsize::new(0);
    let cpu_id = CPU_ID_GEN.fetch_add(1, Ordering::Relaxed);
    unsafe {
        set_cpu_id(cpu_id);
    }

    int::disable();

    if !cpu.is_boot() {
        // bsp cpu structs are already initialized
        cpu::init(cpu);
    }

    hyperion_drivers::acpi::init();

    // int::enable();
}

pub fn cpu_count() -> usize {
    hyperion_boot::cpu_count()
}

pub fn cpu_id() -> usize {
    let tsc = Msr::new(IA32_TSC_AUX);
    unsafe { tsc.read() as _ }
}

/// # Safety
///
/// id's should be unique to each CPU
pub unsafe fn set_cpu_id(id: usize) {
    let mut tsc = Msr::new(IA32_TSC_AUX);
    unsafe { tsc.write(id as _) }
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

#[inline(always)]
pub fn dbg_cpu() {
    let rsp: u64;
    unsafe {
        core::arch::asm!("mov {rsp}, rsp", rsp = lateout(reg) rsp);
    }

    let rip = x86_64::instructions::read_rip();

    let cr3 = x86_64::registers::control::Cr3::read().0.start_address();

    hyperion_log::debug!("rsp:0x{rsp:0x} rip:0x{rip:0x} cr3:0x{cr3:0x}");
}
