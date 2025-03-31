#![no_std]
#![feature(abi_x86_interrupt, naked_functions, cell_leak)]

//

extern crate alloc;

use core::{arch::asm, ptr::NonNull};

use hyperion_log::*;
use x86_64::{
    instructions::random::RdRand,
    registers::{
        control::{Cr0, Cr0Flags, Cr4, Cr4Flags},
        model_specific::GsBase,
    },
    structures::idt::InterruptStackFrame,
    PrivilegeLevel,
};

use self::{syscall::SyscallHandler, tls::ThreadLocalStorage};

//

pub mod context;
pub mod cpu;
pub mod paging;
// pub mod stack;
pub mod syscall;
pub mod tls;
pub mod vmm;

//

pub fn init(handler: SyscallHandler) {
    int::disable();

    // init TSS, IDT, GDT and cpu local storage
    let tls = cpu::init();

    init_features();

    // deep copy the kernel mapping(s)
    vmm::init();

    // init syscall and sysret
    syscall::init(tls.cpu.gdt.selectors, handler);
}

pub fn swapgs_guard(f: &InterruptStackFrame) -> impl Drop {
    struct Guard(bool);

    impl Drop for Guard {
        fn drop(&mut self) {
            if self.0 {
                swapgs();
            }
        }
    }

    let swap = f.code_segment.rpl() != PrivilegeLevel::Ring0;
    if swap {
        swapgs();
    }
    Guard(swap)
}

pub fn swapgs() {
    unsafe { asm!("swapgs") };
}

pub fn cpu_local() -> &'static ThreadLocalStorage {
    debug_assert_ne!(GsBase::read().as_u64(), 0);

    let ptr: *mut ThreadLocalStorage;
    unsafe { asm!("mov {}, gs:0", out(reg) ptr) };
    let ptr = NonNull::new(ptr).expect("cpu_id not set up");
    unsafe { ptr.as_ref() }
}

pub fn cpu_id() -> usize {
    cpu_local().cpu_id
}

#[inline(always)]
pub extern "C" fn reset_rbp() {
    unsafe {
        // asm!("mov QWORD PTR [rbp], 0", "mov QWORD PTR [rbp+8], 0");
    }
}

fn init_features() {
    let res = unsafe { core::arch::x86_64::__cpuid(0x1) };
    if res.edx & (1 << 25) == 0 {
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

pub fn wake_cpus(start: extern "C" fn() -> !) {
    hyperion_boot::smp_init(cpu_id() == 0, start);
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
        int::disable();
        x86_64::instructions::hlt();
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
