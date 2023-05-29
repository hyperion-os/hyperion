//! High Precision Event Timer
//!
//! https://wiki.osdev.org/HPET

use core::ptr::{read_volatile, write_volatile};

use bit_field::BitField;
use chrono::Duration;
use smallvec::SmallVec;
use spin::{Lazy, Mutex, MutexGuard};

use super::{rsdt::RSDT, SdtError};
use crate::{
    debug, trace,
    util::slice_read::slice_read,
    vfs::{FileDevice, IoError, IoResult},
};

//

pub static HPET: Lazy<Mutex<Hpet>> = Lazy::new(|| Mutex::new(Hpet::init()));

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
    next_timer: u8,
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
            next_timer: 0,
            timers: <_>::default(),
        };

        res.init_self();

        Ok(res)
    }

    //

    pub fn timer(&mut self, n: u8) -> TimerN {
        todo!()
        /* assert!(n <= self.timers);
        TimerN {
            hpet: self,
            offs: 0x100 + 0x20 * n as u64,
        } */
    }

    pub fn next_timer(&self) -> MutexGuard<'_, TimerN> {
        self.timers
            .iter()
            .cycle()
            .skip(self.next_timer as _)
            // .take(self.timers.len())
            .find_map(|timer| timer.try_lock())
            .unwrap()
    }

    //

    pub fn caps(&mut self) -> GeneralCaps {
        GeneralCaps(Hpet::read_reg(self.addr, 0x000))
    }

    pub fn config(&mut self) -> GeneralConfig {
        GeneralConfig(Hpet::read_reg(self.addr, 0x010))
    }

    pub fn set_config(&mut self, config: GeneralConfig) {
        Hpet::write_reg(self.addr, 0x010, config.0)
    }

    pub fn interrupt_status(&mut self) -> GeneralInterruptStatus {
        GeneralInterruptStatus(Hpet::read_reg(self.addr, 0x020))
    }

    pub fn set_interrupt_status(&mut self, status: GeneralInterruptStatus) {
        Hpet::write_reg(self.addr, 0x020, status.0)
    }

    pub fn main_counter_value(&mut self) -> CounterValue {
        Hpet::read_reg(self.addr, 0x0F0)
    }

    pub fn set_main_counter_value(&mut self, val: CounterValue) {
        Hpet::write_reg(self.addr, 0x0F0, val)
    }

    //

    /// theoretical max u96 sized output
    pub fn femtos(&mut self) -> u128 {
        self.period as u128 * self.main_counter_value() as u128
    }

    /// theoretical max u87 sized output
    pub fn picos(&mut self) -> u128 {
        self.femtos() / 1_000
    }

    /// theoretical max u77 sized output
    pub fn nanos(&mut self) -> u128 {
        self.picos() / 1_000
    }

    /// theoretical max u67 sized output
    pub fn micros(&mut self) -> u128 {
        self.nanos() / 1_000
    }

    /// theoretical max u57 sized output
    pub fn millis(&mut self) -> u64 {
        (self.micros() / 1_000) as u64
    }

    /// theoretical max u47 sized output
    pub fn seconds(&mut self) -> u64 {
        self.millis() / 1_000
    }

    /// theoretical max u41 sized output
    pub fn minutes(&mut self) -> u64 {
        self.millis() / 60
    }

    pub fn now_bytes(&mut self) -> [u8; 16] {
        self.femtos().to_le_bytes()
    }

    //

    fn read_reg(addr: u64, reg: u64) -> u64 {
        unsafe { read_volatile((addr + reg) as *const u64) }
    }

    fn write_reg(addr: u64, reg: u64, val: u64) {
        unsafe { write_volatile((addr + reg) as *mut u64, val) }
    }

    fn init_self(&mut self) {
        let caps = self.caps();
        self.period = caps.period() as u32;

        let timers = caps.num_tim_cap();
        debug!("HPET comparator count: {timers}");
        for timer in 0..timers {
            let mut timer = TimerN {
                addr: self.addr + 0x100 + 0x20 * timer,
            };
            timer.init();
            self.timers.push(Mutex::new(timer));
        }

        // enable cnf => enable hpet
        let mut config = self.config();
        config.set_enable(1);
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
    /// non blocking sleep, this triggers an interrupt after `dur`
    pub fn sleep(&mut self, dur: Duration) {}

    pub fn init(&mut self) {
        let mut config = self.config_and_caps();
        config.set_int_enable(1);
        self.set_config_and_caps(config);
    }

    //

    pub fn config_and_caps(&mut self) -> TimerNConfigAndCaps {
        TimerNConfigAndCaps(Hpet::read_reg(self.addr, 0x0))
    }

    pub fn set_config_and_caps(&mut self, val: TimerNConfigAndCaps) {
        Hpet::write_reg(self.addr, 0x0, val.0)
    }

    pub fn comparator_value(&mut self) -> CounterValue {
        Hpet::read_reg(self.addr, 0x8)
    }

    pub fn set_comparator_value(&mut self, val: CounterValue) {
        Hpet::write_reg(self.addr, 0x8, val)
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
        let bytes = &HPET.lock().now_bytes()[..];
        slice_read(bytes, offset, buf)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

//

macro_rules! bitfield {
    ($name:ident = $t:ty { $($field:ident : $range:expr),* $(,)? }) => {
        ::paste::paste! {
            #[derive(Clone, Copy)]
            pub struct $name($t);

            impl $name {
                $(
                    pub fn $field(&self) -> $t {
                        self.0.get_bits($range)
                    }

                    pub fn [<set_ $field>](&mut self, val: $t) {
                        self.0.set_bits($range, val);
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
        }
    };

    ($($name:ident = $t:ty { $($other:tt)* })*) => {
        $(
            bitfield! {
                $name = $t { $($other)* }
            }
        )*
    };
}

bitfield! {
    GeneralCaps = u64 {
        period: 32..64,
        vendor_id: 16..32,
        leg_rt_cap: 15..16,
        count_size_cap: 14..15,
        num_tim_cap: 8..14,
        rev_id: 0..8,
    }

    GeneralConfig = u64 {
        leg_rt: 1..2,
        enable: 0..1,
    }

    TimerNConfigAndCaps = u64 {
        int_route_cap: 32..64,
        fsb_int_del_cap: 15..16,
        fsb_enable: 14..15,
        int_route: 9..14,
        value_set: 6..7,
        size_cap: 5..6,
        per_int_cap: 4..5,
        mode: 3..4,
        int_enable: 2..3,
        int_trigger: 1..2,
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
        comparator_count: 0..5,
        counter_size: 5..6,
        reserved: 6..7,
        legacy_replacement: 7..8
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
