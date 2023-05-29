//! High Precision Event Timer
//!
//! https://wiki.osdev.org/HPET

use core::{
    ptr::{read_volatile, write_volatile},
    sync::atomic::{AtomicU8, Ordering},
};

use bit_field::BitField;
use chrono::Duration;
use smallvec::SmallVec;
use spin::{Lazy, Mutex, MutexGuard};

use super::{rsdt::RSDT, SdtError};
use crate::{
    arch::cpu::idt::Irq,
    debug,
    driver::acpi::{apic::ApicId, ioapic::IoApic},
    scheduler::timer::provide_sleep_wake,
    trace,
    util::slice_read::slice_read,
    vfs::{FileDevice, IoError, IoResult},
    warn,
};

//

pub static HPET: Lazy<Hpet> = Lazy::new(Hpet::init);

//

#[derive(Debug)]
pub struct Hpet {
    addr: u64,

    // minimum_tick: u16,
    /// HPET period in femtoseconds
    period: u32,
    // vendor_id: u16,
    // leg_rt_cap: bool,
    // count_size_cap: bool,
    // rev_id: u8,
    next_timer: AtomicU8,
    timers: SmallVec<[Mutex<TimerN>; 34]>,
}

#[derive(Debug)]
pub struct HpetRegs {
    // general_caps: Reg<1, ReadOnly, GeneralCaps>,
    // general_config: Reg<1, ReadWrite, GeneralConfig>,
    // general_interrupt_status: Reg<1, ReadWrite, GeneralInterruptStatus>,
    // main_counter_value: Reg<1, ReadWrite, MainCounterValue>,
}

#[derive(Debug)]
pub struct TimerN {
    addr: u64,
    offs: u64,
    handler: Option<ApicId>,
}

#[derive(Debug, Clone, Copy)]
pub enum HpetError {
    Sdt(SdtError),
    DoesntExist,
}

//

impl Hpet {
    pub fn init() -> Self {
        Self::try_init().expect("HPET should be valid")
    }

    pub fn try_init() -> Result<Self, HpetError> {
        let (_, mut unpacker) = RSDT.find_table(*b"HPET").ok_or(HpetError::DoesntExist)?;
        let u = &mut unpacker;

        let hpet: RawHpet = u.unpack(true)?;

        trace!("HPET initialized {hpet:#x?}");

        let mut res = Self {
            addr: hpet.address.address,
            // minimum_tick: hpet.minimum_tick,
            period: 0,
            next_timer: AtomicU8::new(0),
            timers: <_>::default(),
        };

        // res.init_self(hpet._bits.comparator_count() as _);
        res.init_self(3);

        Ok(res)
    }

    //

    /// handles a timer interrupt
    pub fn int_ack(&self) {
        provide_sleep_wake()
    }

    //

    pub fn next_timer(&self) -> MutexGuard<'_, TimerN> {
        self.timers
            .iter()
            .cycle()
            .skip(self.next_timer.load(Ordering::Relaxed) as usize % self.timers.len())
            .map(|t| {
                self.next_timer.fetch_add(1, Ordering::Relaxed);
                t
            })
            // .take(self.timers.len())
            .find_map(|timer| timer.try_lock())
            .unwrap()
    }

    pub fn to_deadline(&self, nanos: u64) -> u64 {
        let main = self.main_counter_value();
        // TODO: integer overflow
        let ticks = nanos * 1_000_000 / HPET.period as u64;
        main + ticks
    }

    //

    pub fn caps(&self) -> GeneralCaps {
        GeneralCaps(Hpet::read_reg(self.addr, 0x000))
    }

    pub fn config(&self) -> GeneralConfig {
        GeneralConfig(Hpet::read_reg(self.addr, 0x010))
    }

    pub fn set_config(&mut self, config: GeneralConfig) {
        Hpet::write_reg(self.addr, 0x010, config.0)
    }

    pub fn interrupt_status(&self) -> GeneralInterruptStatus {
        GeneralInterruptStatus(Hpet::read_reg(self.addr, 0x020))
    }

    pub fn set_interrupt_status(&mut self, status: GeneralInterruptStatus) {
        Hpet::write_reg(self.addr, 0x020, status.0)
    }

    pub fn main_counter_value(&self) -> CounterValue {
        Hpet::read_reg(self.addr, 0x0F0)
    }

    pub fn set_main_counter_value(&mut self, val: CounterValue) {
        Hpet::write_reg(self.addr, 0x0F0, val)
    }

    //

    /// theoretical max u96 sized output
    pub fn femtos(&self) -> u128 {
        self.period as u128 * self.main_counter_value() as u128
    }

    /// theoretical max u87 sized output
    pub fn picos(&self) -> u128 {
        self.femtos() / 1_000
    }

    /// theoretical max u77 sized output
    pub fn nanos(&self) -> u128 {
        self.picos() / 1_000
    }

    /// theoretical max u67 sized output
    pub fn micros(&self) -> u128 {
        self.nanos() / 1_000
    }

    /// theoretical max u57 sized output
    pub fn millis(&self) -> u64 {
        (self.micros() / 1_000) as u64
    }

    /// theoretical max u47 sized output
    pub fn seconds(&self) -> u64 {
        self.millis() / 1_000
    }

    /// theoretical max u41 sized output
    pub fn minutes(&self) -> u64 {
        self.millis() / 60
    }

    pub fn now_bytes(&self) -> [u8; 16] {
        self.femtos().to_le_bytes()
    }

    //

    fn read_reg(addr: u64, reg: u64) -> u64 {
        unsafe { read_volatile((addr + reg) as *const u64) }
    }

    fn write_reg(addr: u64, reg: u64, val: u64) {
        unsafe { write_volatile((addr + reg) as *mut u64, val) }
    }

    fn init_self(&mut self, cmp_count: u64) {
        let caps = self.caps();
        self.period = caps.period() as u32;

        let timers = (caps.num_tim_cap() - 1).min(cmp_count);
        debug!("HPET timer count: {timers}");
        for timer in 0..timers {
            let mut timer = TimerN {
                addr: self.addr,
                offs: 0x100 + 0x20 * timer,
                handler: None,
            };
            timer.init();
            self.timers.push(Mutex::new(timer));
        }

        // enable cnf => enable hpet
        let mut config = self.config();
        config.set_enable(true);
        self.set_config(config);

        debug!("HPET caps: {:#x?}", self.caps());
        debug!("HPET config: {:#x?}", self.config());
        debug!("HPET int status: {:#x?}", self.interrupt_status());
        debug!("HPET freq: {}", Self::freq(self.period));
    }

    #[allow(unused)]
    const fn freq(period: u32) -> u32 {
        (10u64.pow(15) / period as u64) as _
        /* const SECOND: u64 = 10u64.pow(15);
        const _100_NANOS: u32 = (SECOND / 10_000_000) as u32;
        _100_NANOS */
    }
}

impl TimerN {
    pub fn handler(&self) -> ApicId {
        self.handler.expect("no I/O APIC handler for HPET timer")
    }

    /// non blocking sleep, this triggers an interrupt after `dur`
    ///
    /// `deadline` is in HPET clock ticks
    ///
    /// if `deadline` is before the current tick, the interrupt never happens
    pub fn sleep_until(&mut self, deadline: u64) {
        // enable interrupts for this timer
        let mut config = self.config_and_caps();
        config.set_int_enable(true);
        self.set_config_and_caps(config);

        // start counting
        self.set_comparator_value(deadline);
    }

    /// non blocking sleep, this triggers an interrupt after `dur`
    pub fn sleep(&mut self, dur: Duration) {
        if let Some(nanos) = dur.num_nanoseconds() {
            if nanos <= 0 {
                return;
            }

            // calculate tick deadline
            let deadline = HPET.to_deadline(nanos as _);
            self.sleep_until(deadline);
        } else {
            todo!();
        }
    }

    pub fn init(&mut self) {
        let mut config = self.config_and_caps();
        config.set_int_route(20); // TODO:
        config.set_int_enable(false);
        self.set_config_and_caps(config);
        self.set_comparator_value(0);

        if let Some(mut ioapic) = IoApic::any() {
            let apic = ioapic.set_irq_any(20, Irq::HpetSleep as _);
            self.handler = Some(apic);
        } else {
            warn!("HPET: no I/O APIC");
        }
    }

    //

    pub fn config_and_caps(&mut self) -> TimerNConfigAndCaps {
        TimerNConfigAndCaps(Hpet::read_reg(self.addr, self.offs))
    }

    pub fn set_config_and_caps(&mut self, val: TimerNConfigAndCaps) {
        Hpet::write_reg(self.addr, self.offs, val.0)
    }

    pub fn comparator_value(&mut self) -> CounterValue {
        Hpet::read_reg(self.addr, self.offs + 0x8)
    }

    pub fn set_comparator_value(&mut self, val: CounterValue) {
        Hpet::write_reg(self.addr, self.offs + 0x8, val)
    }
}

impl From<SdtError> for HpetError {
    fn from(value: SdtError) -> Self {
        Self::Sdt(value)
    }
}

//

pub struct HpetDevice;

//

impl FileDevice for HpetDevice {
    fn len(&self) -> usize {
        core::mem::size_of::<i64>()
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        let bytes = &HPET.now_bytes()[..];
        slice_read(bytes, offset, buf)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

//

macro_rules! bitfield {
    ($name:ident = $t:ty { $($(#[$($field_docs:meta)*])* $field:ident : $range:expr),* $(,)? }) => {
            #[derive(Clone, Copy)]
            pub struct $name($t);

            impl $name {
                $(
                    bitfield! { impl
                        $(#[$($field_docs)*])*
                        $field : $t = $range
                    }
                 )*
            }

            impl ::core::fmt::Debug for $name {
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    f.debug_struct(stringify!($name))
                        $(
                            .field(stringify!($field), &self.$field())
                         )*
                        .finish()
                }
            }
    };

    ($($name:ident = $t:ty { $($other:tt)* })*) => {
        $(
            bitfield! {
                $name = $t { $($other)* }
            }
        )*
    };

    (impl $(#[$($field_docs:meta)*])* $field:ident : $t:ty = $bit:literal) => {::paste::paste! {
        $(#[$($field_docs)*])*
        pub fn $field(&self) -> bool {
            self.0.get_bit($bit)
        }

        $(#[$($field_docs)*])*
        pub fn [<set_ $field>](&mut self, val: bool) {
            self.0.set_bit($bit, val);
        }
    }};

    (impl $(#[$($field_docs:meta)*])* $field:ident : $t:ty = $range:expr) => {::paste::paste! {
        $(#[$($field_docs)*])*
        pub fn $field(&self) -> $t {
            self.0.get_bits($range)
        }

        $(#[$($field_docs)*])*
        pub fn [<set_ $field>](&mut self, val: $t) {
            self.0.set_bits($range, val);
        }
    }};
}

bitfield! {
    GeneralCaps = u64 {
        /// main counter tick period in femtoseconds
        period: 32..64,

        /// PCI vendor ID
        vendor_id: 16..32,

        /// has legacy replacement mapping capability?
        leg_rt_cap: 15,

        /// has 64 bit mode capability?
        count_size_cap: 14,

        /// number of timers - 1
        num_tim_cap: 8..14,

        /// implementation revision ID
        rev_id: 0..8,
    }

    GeneralConfig = u64 {
        /// legacy replacement mapping
        leg_rt: 1,

        /// enable HPET
        enable: 0,
    }

    TimerNConfigAndCaps = u64 {
        /// interrupt routing capability
        ///
        /// bit X = IRQX I/O APIC mapping capability
        int_route_cap: 32..64,

        /// has front side bus interrupt mapping capability?
        fsb_int_del_cap: 15..16,

        /// front side bus interrupt mapping enabled
        fsb_enable: 14..15,

        /// I/O APIC routing [`int_route_cap`]
        int_route: 9..14,

        /// write to periodic timer's accumulator
        value_set: 6,

        /// has 64 bit mode capability?
        size_cap: 5,

        /// has periodic mode capability?
        periodic_int_cap: 4,

        /// active mode
        /// 0 = one-shot (non-peridic)
        /// 1 = periodic
        mode: 3,

        /// enable interrupts
        int_enable: 2,

        /// interrupt trigger mode
        /// 0 = edge-triggered
        /// 1 = level-triggered
        int_trigger: 1,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GeneralInterruptStatus(u64);

impl GeneralInterruptStatus {
    pub fn timer_n_int_status(self, n: usize) -> bool {
        self.0.get_bits(n..(n + 1)) != 0
    }
}

pub type CounterValue = u64;

#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct RawHpet {
    /* hw_id: u32, */
    hardware_rev_id: u8,
    _bits: RawHpetBits,
    pci_vendor_id: u16,
    address: Address,
    hpet_number: u8,
    minimum_tick: u16,
    page_protection: u8,
}

bitfield! {
    RawHpetBits = u8 {
        comparator_count: 3..8,
        counter_size: 2,
        reserved: 1,
        legacy_replacement: 0,
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct Address {
    address_space_id: u8,
    register_bit_width: u8,
    register_bit_offset: u8,
    access_width: u8,
    address: u64,
}

impl Address {
    #[allow(unused)]
    pub fn is_system_memory(&self) -> bool {
        self.address_space_id == 0
    }

    #[allow(unused)]
    pub fn is_system_io(&self) -> bool {
        self.address_space_id == 1
    }
}
