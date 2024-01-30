#![no_std]
#![feature(
    abi_x86_interrupt,
    naked_functions,
    new_uninit,
    asm_const,
    const_refs_to_cell,
    cell_leak
)]

//

extern crate alloc;

use hyperion_cpu_id::{self as cpu_id, cpu_id};
use hyperion_log::*;
use x86_64::{
    instructions::random::RdRand,
    registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags},
};

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

pub fn init() {
    int::disable();

    // set this CPUs id using an atomic inc variable
    cpu_id::init();

    // init TSS, IDT, GDT
    cpu::init();

    init_features();
}

fn init_features() {
    let res = unsafe { core::arch::x86_64::__cpuid(0x1) };
    if res.edx & 1 << 25 == 0 {
        panic!("No SSE HW support");
    }

    let mut cr0 = Cr0::read();
    cr0.remove(Cr0Flags::EMULATE_COPROCESSOR);
    cr0.insert(Cr0Flags::MONITOR_COPROCESSOR);
    unsafe { Cr0::write(cr0) };

    let mut cr4 = Cr4::read();
    cr4.insert(Cr4Flags::OSFXSR | Cr4Flags::OSXMMEXCPT_ENABLE);
    // cr4.insert(Cr4Flags::FSGSBASE);
    unsafe { Cr4::write(cr4) };
}

pub fn wake_cpus() {
    if cpu_id() == 0 {
        hyperion_boot::smp_init();
    }

    hyperion_drivers::acpi::init();
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

    pub extern "C" fn enable_and_nop64() {
        enable();
        for _ in 0..64 {
            x86_64::instructions::nop();
        }
        disable();
    }

    pub extern "C" fn wait() {
        // extern "C" disables red zones and red zones fuck up the stack when an interrupt happens
        // https://doc.rust-lang.org/rustc/platform-support/x86_64-unknown-none.html
        int::enable_and_hlt();
        disable();
    }
}

pub fn spin_loop() {
    core::hint::spin_loop()
}

/// `HCF` - halt the cpu forever
pub fn die() -> ! {
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

    let ints = int::are_enabled();

    hyperion_log::debug!("rsp:0x{rsp:0x} rip:0x{rip:0x} cr3:0x{cr3:0x} ints:{ints}");
}
