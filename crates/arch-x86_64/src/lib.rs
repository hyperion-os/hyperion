#![no_std]
#![feature(
    abi_x86_interrupt,
    custom_test_frameworks,
    naked_functions,
    new_uninit,
    asm_const,
    const_refs_to_cell,
    cell_leak
)]
#![forbid(unsafe_op_in_unsafe_fn)]

//

extern crate alloc;

use core::ops::Range;

use hyperion_cpu_id::{self as cpu_id, cpu_id};
use hyperion_log::error;
use x86_64::{instructions::random::RdRand, VirtAddr};

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
}

pub fn wake_cpus() {
    if cpu_id() == 0 {
        hyperion_boot::smp_init();
    }

    hyperion_drivers::acpi::init();
}

pub fn stack_pages() -> Range<VirtAddr> {
    let rsp: u64;
    unsafe {
        core::arch::asm!("mov {rsp}, rsp", rsp = lateout(reg) rsp);
    }

    let top = VirtAddr::new(rsp).align_up(0x1000u64);
    let bottom = top - hyperion_boot::BOOT_STACK_SIZE;

    debug_assert!(bottom.is_aligned(0x1000u64));

    bottom..top
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

    let ints = int::are_enabled();

    hyperion_log::debug!("rsp:0x{rsp:0x} rip:0x{rip:0x} cr3:0x{cr3:0x} ints:{ints}");
}
