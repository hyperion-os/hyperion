use super::LOCAL_APIC;
use crate::{arch::cpu::idt::Irq, debug};
use core::{fmt, marker::PhantomData, ptr};
use spin::{Lazy, Mutex, MutexGuard};

//

pub fn apic_regs() -> MutexGuard<'static, &'static mut ApicRegs> {
    pub static APIC_REGS: Lazy<Mutex<&'static mut ApicRegs>> =
        Lazy::new(|| Mutex::new(unsafe { &mut *(*LOCAL_APIC as *mut ApicRegs) }));

    APIC_REGS.lock()
}

//

const IA32_APIC_BASE: u32 = 0x1B;
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

pub fn enable() {
    debug!("Writing ENABLE APIC");
    write_msr(
        IA32_APIC_BASE,
        read_msr(IA32_APIC_BASE) | IA32_APIC_XAPIC_ENABLE,
    );

    let apic_regs = unsafe { &mut *(*LOCAL_APIC as *mut ApicRegs) };
    // debug!("Apic regs: {apic_regs:#?}");

    // reset to well-known state
    apic_regs.destination_format.write(0xFFFF_FFFF);
    apic_regs.lvt_timer.write(APIC_DISABLE);
    apic_regs.lvt_perf_mon_counters.write(APIC_NMI);
    apic_regs.lvt_lint_0.write(APIC_DISABLE);
    apic_regs.lvt_lint_1.write(APIC_DISABLE);
    apic_regs.task_priority.write(0);

    debug!("Writing ENABLE SIVR");
    apic_regs
        .spurious_interrupt_vector
        .write(apic_regs.spurious_interrupt_vector.read() | APIC_SW_ENABLE);

    /*     let apic_period = 1000000; */
    let apic_period = u32::MAX;

    apic_regs.timer_divide.write(APIC_TIMER_DIV);
    apic_regs
        .lvt_timer
        .write(Irq::ApicTimer as u32 | APIC_TIMER_MODE_PERIODIC);
    apic_regs.timer_init.write(apic_period);

    apic_regs.lvt_thermal_sensor.write(0);
    apic_regs.lvt_error.write(0);

    // buggy HW fix:
    apic_regs.timer_divide.write(APIC_TIMER_DIV);

    // loop { /* debug!("APIC TIMER {}", apic_regs.timer_current.read()); */ }
}

/* fn read_apic_reg(reg: usize) -> u32 {
    unsafe { ptr::read_volatile((*LOCAL_APIC + reg) as _) }
}

fn write_apic_reg(reg: usize, val: u32) {
    unsafe { ptr::write_volatile((*LOCAL_APIC + reg) as _, val) }
} */

fn read_msr(msr: u32) -> u64 {
    unsafe { x86_64::registers::model_specific::Msr::new(msr).read() }
}

fn write_msr(msr: u32, val: u64) {
    unsafe { x86_64::registers::model_specific::Msr::new(msr).write(val) }
}

//

/// Table 10-1 Local APIC Register Address Map
///
/// https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-3a-part-1-manual.pdf
///
/// 10-6 Vol. 3A
#[derive(Debug)]
#[repr(C)]
pub struct ApicRegs {
    pub _res0: [Reg; 2],
    pub lapic_id: Reg<ReadWrite>,
    pub lapic_ver: Reg<Read>,
    pub _res1: [Reg; 4],
    pub task_priority: Reg<ReadWrite>,
    pub arbitration_priority: Reg<Read>,
    pub processor_priority: Reg<Read>,
    pub eoi: Reg<Write>,
    pub remote_read: Reg<Read>,
    pub logical_destination: Reg<ReadWrite>,
    pub destination_format: Reg<ReadWrite>,
    pub spurious_interrupt_vector: Reg<ReadWrite>,
    pub _pad2: [Reg; 34],
    pub lvt_timer: Reg<ReadWrite>,
    pub lvt_thermal_sensor: Reg<ReadWrite>,
    pub lvt_perf_mon_counters: Reg<ReadWrite>,
    pub lvt_lint_0: Reg<ReadWrite>,
    pub lvt_lint_1: Reg<ReadWrite>,
    pub lvt_error: Reg<ReadWrite>,
    pub timer_init: Reg<ReadWrite>,
    pub timer_current: Reg<Read>,
    pub _res2: Reg,
    pub timer_divide: Reg<ReadWrite>,
}

#[repr(C)]
pub struct Reg<A = ()> {
    val: u32,
    _pad: [u32; 3],
    _p: PhantomData<A>,
}

pub struct Read;
pub struct ReadWrite;
pub struct Write;

//

impl Reg<Read> {
    pub fn read(&self) -> u32 {
        unsafe { ptr::read_volatile(&self.val as _) }
    }
}

impl Reg<ReadWrite> {
    pub fn read(&self) -> u32 {
        unsafe { ptr::read_volatile(&self.val as _) }
    }

    pub fn write(&mut self, val: u32) {
        unsafe { ptr::write_volatile(&mut self.val as _, val) }
    }
}

impl Reg<Write> {
    pub fn write(&mut self, val: u32) {
        unsafe { ptr::write_volatile(&mut self.val as _, val) }
    }
}

impl fmt::Debug for Reg<Read> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.read(), f)
    }
}

impl fmt::Debug for Reg<ReadWrite> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.read(), f)
    }
}

impl fmt::Debug for Reg<Write> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt("<NO READS>", f)
    }
}

impl fmt::Debug for Reg<()> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt("<NO READS>", f)
    }
}
