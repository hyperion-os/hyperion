use alloc::boxed::Box;
use core::ops::Deref;

use crossbeam::atomic::AtomicCell;
use hyperion_atomic_map::AtomicMap;
use hyperion_interrupts::{end_of_interrupt, IntController, INT_CONTROLLER, INT_EOI_HANDLER};
use hyperion_log::trace;
use hyperion_mem::to_higher_half;
use spin::{Lazy, RwLock, RwLockReadGuard, RwLockWriteGuard};
use x86_64::PhysAddr;

use super::{madt::MADT, ReadOnly, ReadWrite, Reserved, WriteOnly};

//

pub static APIC_TIMER_HANDLER: AtomicCell<fn()> = AtomicCell::new(|| {});

pub const IRQ_APIC_SPURIOUS: u8 = 0xFF;
pub const APIC_CALIBRATION_MICROS: u16 = 10_000;
pub const APIC_PERIOD_MULT: u32 = 1;

//

pub struct ApicTls<T: 'static> {
    inner: Box<[(ApicId, T)]>,
}

impl<T: 'static> ApicTls<T> {
    pub fn new(mut f: impl FnMut() -> T) -> Self {
        let mut inner: Box<[(ApicId, T)]> = ApicId::iter().map(|id| (id, f())).collect();

        inner.sort_by_key(|(id, _)| *id);

        Self { inner }
    }
}

impl<T: 'static> Deref for ApicTls<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let key = ApicId::current();
        let idx = self
            .inner
            .binary_search_by_key(&key, |(id, _)| *id)
            .unwrap_or_else(|_| panic!("{key:?} was expected to be a registered LAPIC"));

        &self.inner[idx].1
    }
}

//

// enable APIC for this processor
pub fn enable() {
    hyperion_interrupts::set_interrupt_handler(IRQ_APIC_SPURIOUS, |irq| {
        // apic spurious interrupt
        // spurdo spärde keskeytys
        end_of_interrupt(irq);
    });

    INT_EOI_HANDLER.store(|_| {
        Lapic::current_mut().eoi();
    });
    INT_CONTROLLER.store(IntController::Apic);

    write_msr(
        IA32_APIC_BASE,
        read_msr(IA32_APIC_BASE) | IA32_APIC_XAPIC_ENABLE,
    );

    // SAFETY: TODO: atm. totally unsafe, because enable could be called twice with the same CPU
    // but this should be the first time ever this CPU checks the apic regs
    let regs: &mut ApicRegs = unsafe { get_apic_regs() };
    let apic_id = ApicId(regs.lapic_id.read());

    trace!("Initializing {apic_id:?}");
    LAPICS.insert(apic_id, RwLock::new(Lapic { regs }));
    let mut lapic = LAPICS.get(&apic_id).unwrap().write();

    const ENABLE_APIC_TASK_SWITCH: bool = true;
    if ENABLE_APIC_TASK_SWITCH {
        enable_timer(lapic);
    } else {
        reset(lapic.regs);
    }

    trace!("Done Initializing {apic_id:?}");
}

pub fn enable_timer(mut lapic: RwLockWriteGuard<Lapic>) {
    let timer_irq = hyperion_interrupts::set_any_interrupt_handler(
        |irq| (0x30..=0xFF).contains(&irq),
        |irq| {
            // hyperion_log::debug!("APIC timer");

            /* unsafe {
                core::arch::asm!(
                    "syscall",
                    in("rax") syscall_id,
                    in("rdi") arg0,
                    in("rsi") arg1,
                    in("rdx") arg2,
                    in("r8") arg3,
                    in("r9") arg4,
                    lateout("rax") result
                );
            } */

            end_of_interrupt(irq);
            APIC_TIMER_HANDLER.load()();

            // apic timer interrupt
        },
    )
    .expect("No avail APIC timer IRQ");

    // let mut lapic = Lapic::current_mut();

    reset(lapic.regs);
    init_lvt_timer(timer_irq, lapic.regs);
}

/// # Safety
///
/// the caller has to make sure there are no other mutable references
/// to the same ApicRegs
pub unsafe fn get_apic_regs() -> &'static mut ApicRegs {
    let lapic_addr = to_higher_half(PhysAddr::new(MADT.local_apic_addr as u64));
    &mut *lapic_addr.as_mut_ptr()
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApicId(u32);

pub struct Lapic {
    regs: &'static mut ApicRegs,
}

//

impl ApicId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn iter() -> impl Iterator<Item = ApicId> {
        // LAPICS.keys().copied()
        LAPIC_IDS.iter().copied()
    }

    pub fn inner(&self) -> u32 {
        self.0
    }

    pub const fn is_ioapic_compatible(self) -> bool {
        self.0 <= 0xFF
    }

    /// apic id of this processor
    pub fn current() -> Self {
        // FIXME: technically UB, because regs could be shared,
        // even though only lapic_id is read, and lapic_id is never allowed
        // to be written
        //
        // but rust wants all mutable refs (the whole &mut ApicRegs here)
        // to be exclusive always
        Self(unsafe { get_apic_regs() }.lapic_id.read())

        // TODO: maybe go with the same solution as Theseus OS
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
static LAPIC_IDS: Lazy<&'static [ApicId]> =
    Lazy::new(|| Box::leak(hyperion_boot::lapics().map(ApicId).collect::<Box<_>>()));

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

    hyperion_log::trace!("apic timer calibration");
    hyperion_clock::get()._apic_sleep_simple_blocking(APIC_CALIBRATION_MICROS, &mut || {
        // reset right before PIT sleeping
        regs.timer_init.write(INITIAL_COUNT);
    });

    regs.lvt_timer.write(APIC_DISABLE);
    let count = INITIAL_COUNT - regs.timer_current.read();

    count * APIC_PERIOD_MULT
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
