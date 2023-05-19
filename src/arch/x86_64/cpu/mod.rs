use self::{gdt::Gdt, idt::Idt, tss::Tss};
use crate::{smp::Cpu, trace};
use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Once;

//

pub mod gdt;
pub mod idt;
pub mod ints;
pub mod tss;

//

pub fn init(cpu: &Cpu) -> CpuState {
    trace!("Loading CpuState for {cpu}");
    let cpu_state = if cpu.is_boot() {
        // boot cpu doesn't need to allocate
        CpuState::new_boot()
    } else {
        // other cpus have to allocate theirs
        CpuState::new()
    };

    if cpu.is_boot() {
        static BOOT_CPUSTATE: AtomicBool = AtomicBool::new(false);
        if BOOT_CPUSTATE.swap(true, Ordering::SeqCst) {
            return cpu_state;
        }
    }

    cpu_state
}

//

#[derive(Clone, Copy)]
pub struct CpuState {
    // tss: &'static Tss,
    // gdt: &'static Gdt,
    // idt: &'static Idt,
}

static BOOT_TSS: Once<Tss> = Once::new();
static BOOT_GDT: Once<Gdt> = Once::new();
static BOOT_IDT: Once<Idt> = Once::new();

impl CpuState {
    fn new_boot() -> Self {
        if BOOT_TSS.get().is_some() && BOOT_GDT.get().is_some() && BOOT_IDT.get().is_some() {
            return Self {};
        }

        let tss = BOOT_TSS.call_once(Tss::new);

        let gdt = BOOT_GDT.call_once(|| Gdt::new(tss));
        gdt.load();

        let idt = BOOT_IDT.call_once(|| Idt::new(tss));
        idt.load();

        Self {}
    }

    fn new() -> Self {
        let tss = Box::leak(Box::new(Tss::new()));
        let gdt = Box::leak(Box::new(Gdt::new(tss)));
        gdt.load();
        let idt = Box::leak(Box::new(Idt::new(tss)));
        idt.load();

        Self {}
    }
}
