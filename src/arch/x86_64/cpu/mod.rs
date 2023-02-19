use self::{gdt::Gdt, idt::Idt, tss::Tss};
use crate::{debug, smp::Cpu, trace};
use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Once;

//

pub mod gdt;
pub mod idt;
pub mod tss;

//

pub fn init(cpu: &Cpu) -> CpuState {
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

    trace!("Loading CpuState for {cpu}");

    cpu_state.gdt.load();
    cpu_state.idt.load();

    debug!("CpuState {:#018x?}", cpu_state.gdt as *const _);
    debug!("CpuState {:#018x?}", cpu_state.idt as *const _);

    cpu_state
}

//

#[derive(Clone, Copy)]
pub struct CpuState {
    tss: &'static Tss,
    gdt: &'static Gdt,
    idt: &'static Idt,
}

static BOOT_TSS: Once<Tss> = Once::new();
static BOOT_GDT: Once<Gdt> = Once::new();
static BOOT_IDT: Once<Idt> = Once::new();

impl CpuState {
    fn new_boot() -> Self {
        if let (Some(tss), Some(gdt), Some(idt)) = (BOOT_TSS.get(), BOOT_GDT.get(), BOOT_IDT.get())
        {
            return Self { tss, gdt, idt };
        }

        let tss = BOOT_TSS.call_once(|| Tss::new());
        let gdt = BOOT_GDT.call_once(|| Gdt::new(tss));
        let idt = BOOT_IDT.call_once(|| Idt::new(tss));

        Self { tss, gdt, idt }
    }

    fn new() -> Self {
        let tss = Box::leak(Box::new(Tss::new()));
        let gdt = Box::leak(Box::new(Gdt::new(tss)));
        let idt = Box::leak(Box::new(Idt::new(tss)));

        Self { tss, gdt, idt }
    }
}
