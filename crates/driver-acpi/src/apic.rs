use hyperion_atomic_map::AtomicMap;
use hyperion_interrupts::{IntController, INT_CONTROLLER, INT_EOI_HANDLER};
use hyperion_log::trace;
use hyperion_mem::to_higher_half;
use spin::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use x86_64::PhysAddr;

use super::{madt::MADT, ReadOnly, ReadWrite, Reserved, WriteOnly};

//

pub const IRQ_APIC_SPURIOUS: u8 = 0xFF;

//

// enable APIC for this processor
pub fn enable() {
    hyperion_interrupts::set_interrupt_handler(IRQ_APIC_SPURIOUS, || {
        // apic spurious interrupt
        // spurdo spärde keskeytys
    });

    write_msr(
        IA32_APIC_BASE,
        read_msr(IA32_APIC_BASE) | IA32_APIC_XAPIC_ENABLE,
    );

    let lapic_addr = to_higher_half(PhysAddr::new(MADT.local_apic_addr as u64));
    let regs: &mut ApicRegs = unsafe { &mut *lapic_addr.as_mut_ptr() };
    let apic_id = ApicId(regs.lapic_id.read());

    trace!("Initializing {apic_id:?}");
    LAPICS.insert(apic_id, RwLock::new(Lapic { regs }));
    let mut lapic = LAPICS.get(&apic_id).unwrap().write();

    reset(lapic.regs);
    // init_lvt_timer(timer_irq, lapic.regs);
    trace!("Done Initializing {apic_id:?}");

    INT_EOI_HANDLER.store(|_| {
        Lapic::current_mut().eoi();
    });
    INT_CONTROLLER.store(IntController::Apic);
}

pub fn enable_timer() {
    let timer_irq = hyperion_interrupts::set_any_interrupt_handler(
        |irq| (0x30..=0xFF).contains(&irq),
        || {
            // apic timer interrupt
        },
    )
    .expect("No avail APIC timer IRQ");

    let mut lapic = Lapic::current_mut();

    reset(lapic.regs);
    init_lvt_timer(timer_irq, lapic.regs);
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApicId(u32);

pub struct Lapic {
    regs: &'static mut ApicRegs,
}

//

impl ApicId {
    pub fn iter() -> impl Iterator<Item = ApicId> {
        LAPICS.keys().copied()
    }

    pub fn inner(&self) -> u32 {
        self.0
    }

    /// apic id of this processor
    pub fn current() -> Self {
        let regs = unsafe { &*(MADT.local_apic_addr as *const ApicRegs) };
        Self(regs.lapic_id.read())
        /* Self(read_msr(IA32_TSC_AUX) as u32) */
    }

    pub fn lapic(&self) -> RwLockReadGuard<'static, Lapic> {
        LAPICS
            .get(self)
            .expect("Invalid ApicID or LAPICS not setup")
            .read()
    }

    pub fn lapic_mut(&self) -> RwLockWriteGuard<'static, Lapic> {
        LAPICS
            .get(self)
            .expect("Invalid ApicID or LAPICS not setup")
            .write()
    }
}

impl Lapic {
    pub fn current() -> RwLockReadGuard<'static, Lapic> {
        ApicId::current().lapic()
    }

    pub fn current_mut() -> RwLockWriteGuard<'static, Lapic> {
        ApicId::current().lapic_mut()
    }

    pub fn regs(&self) -> &ApicRegs {
        self.regs
    }

    pub fn regs_mut(&mut self) -> &mut ApicRegs {
        self.regs
    }

    pub fn eoi(&mut self) {
        self.regs.eoi.write(0);
    }
}

//

static LAPICS: AtomicMap<ApicId, RwLock<Lapic>> = AtomicMap::new();

const IA32_APIC_BASE: u32 = 0x1B;
// const IA32_TSC_AUX: u32 = 0xC0000103; // lapic id storage - same as in Theseus

const IA32_APIC_XAPIC_ENABLE: u64 = 1 << 11;
const _IA32_APIC_X2APIC_ENABLE: u64 = 1 << 10;

const APIC_SW_ENABLE: u32 = 1 << 8;
const APIC_DISABLE: u32 = 1 << 16;

const APIC_NMI: u32 = 4 << 8;

const _APIC_TIMER_MODE_ONESHOT: u32 = 0b00 << 17;
const APIC_TIMER_MODE_PERIODIC: u32 = 0b01 << 17;
const _APIC_TIMER_MODE_TSC_DEADLINE: u32 = 0b10 << 17;

const _APIC_TIMER_DIV_BY_1: u32 = 0b1011;
const _APIC_TIMER_DIV_BY_2: u32 = 0b0000;
const _APIC_TIMER_DIV_BY_4: u32 = 0b0001;
const _APIC_TIMER_DIV_BY_8: u32 = 0b0010;
const APIC_TIMER_DIV_BY_16: u32 = 0b0011;
const _APIC_TIMER_DIV_BY_32: u32 = 0b1000;
const _APIC_TIMER_DIV_BY_64: u32 = 0b1001;
const _APIC_TIMER_DIV_BY_128: u32 = 0b1010;
const APIC_TIMER_DIV: u32 = APIC_TIMER_DIV_BY_16;

//

fn reset(regs: &mut ApicRegs) {
    // reset to well-known state
    // TODO: fix this bug in rust-analyzer:
    // both next lines work with rustc, but only the commented line works in rust-analyzer
    // Reg::<ReadWrite>::write(&mut apic_regs.destination_format, 0xFFFF_FFFF);
    regs.destination_format.write(0xFFFF_FFFF);
    regs.logical_destination
        .write(regs.logical_destination.read() & 0x00FF_FFFF);
    regs.lvt_timer.write(APIC_DISABLE);
    regs.lvt_perf_mon_counters.write(APIC_NMI);
    regs.lvt_lint_0.write(APIC_DISABLE);
    regs.lvt_lint_1.write(APIC_DISABLE);
    regs.task_priority.write(0);

    // enable interrupts
    regs.spurious_interrupt_vector.write(0xFF + APIC_SW_ENABLE);
}

fn init_lvt_timer(timer_irq: u8, regs: &mut ApicRegs) {
    // let apic_period = 1_000_000;
    let apic_period = calibrate(regs);

    regs.timer_divide.write(APIC_TIMER_DIV);
    regs.lvt_timer
        .write(timer_irq as u32 | APIC_TIMER_MODE_PERIODIC);
    regs.timer_init.write(apic_period);

    regs.lvt_thermal_sensor.write(0);
    regs.lvt_error.write(0);

    // buggy HW fix:
    regs.timer_divide.write(APIC_TIMER_DIV);
}

fn calibrate(regs: &mut ApicRegs) -> u32 {
    const INITIAL_COUNT: u32 = 0xFFFF_FFFF;

    regs.timer_divide.write(APIC_TIMER_DIV);

    hyperion_clock::get()._apic_sleep_simple_blocking(10_000, &mut || {
        // reset right before PIT sleeping
        regs.timer_init.write(INITIAL_COUNT);
    });

    regs.lvt_timer.write(APIC_DISABLE);
    INITIAL_COUNT - regs.timer_current.read()
}

fn read_msr(msr: u32) -> u64 {
    unsafe { x86_64::registers::model_specific::Msr::new(msr).read() }
}

fn write_msr(msr: u32, val: u64) {
    unsafe { x86_64::registers::model_specific::Msr::new(msr).write(val) }
}

//

type Skip<const N: usize> = Reserved<[u32; N]>;

/// Table 10-1 Local APIC Register Address Map
///
/// https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-3a-part-1-manual.pdf
///
/// 10-6 Vol. 3A
#[derive(Debug)]
#[repr(C)]
pub struct ApicRegs {
    _res0: Skip<2>,
    pub lapic_id: ReadWrite,
    pub lapic_ver: ReadOnly,
    _res1: Skip<4>,
    pub task_priority: ReadWrite,
    pub arbitration_priority: ReadOnly,
    pub processor_priority: ReadOnly,
    pub eoi: WriteOnly,
    pub remote_read: ReadOnly,
    pub logical_destination: ReadWrite,
    pub destination_format: ReadWrite,
    pub spurious_interrupt_vector: ReadWrite,
    /* pub in_service: ReadOnly<[u32; 8]>,
    pub trigger_mode: ReadOnly<[u32; 8]>,
    pub interrupt_request: ReadOnly<[u32; 8]>,
    pub error_status: ReadOnly,
    _pad2: Skip<6>,
    pub lvt_corrected_machine_check_interrupt: ReadWrite,
    pub interrupt_cmd: ReadWrite<[u32; 2]>, */
    _pad2: Skip<34>,
    pub lvt_timer: ReadWrite,
    pub lvt_thermal_sensor: ReadWrite,
    pub lvt_perf_mon_counters: ReadWrite,
    pub lvt_lint_0: ReadWrite,
    pub lvt_lint_1: ReadWrite,
    pub lvt_error: ReadWrite,
    pub timer_init: ReadWrite,
    pub timer_current: ReadOnly,
    _res2: Skip<1>,
    pub timer_divide: ReadWrite,
}
