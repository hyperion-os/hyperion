//! High Precision Event Timer
//!
//! https://wiki.osdev.org/HPET

use core::ptr::{read_volatile, write_volatile};

use bit_field::BitField;
use chrono::Duration;
use spin::{Lazy, Mutex};

use crate::{
    debug, trace,
    util::slice_read::{self, slice_read, slice_write},
    vfs::{FileDevice, IoError, IoResult},
};

use super::{rsdt::RSDT, SdtError};

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
    timers: u8,
    // rev_id: u8,
}

#[derive(Debug)]
pub struct HpetRegs {
    // general_caps: Reg<1, ReadOnly, GeneralCaps>,
    // general_config: Reg<1, ReadWrite, GeneralConfig>,
    // general_interrupt_status: Reg<1, ReadWrite, GeneralInterruptStatus>,
    // main_counter_value: Reg<1, ReadWrite, MainCounterValue>,
}

#[derive(Debug)]
pub struct TimerN<'a> {
    hpet: &'a mut Hpet,
    offs: u64,
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
            timers: 0,
        };

        res.init_self();

        Ok(res)
    }

    //

    pub fn timer(&mut self, n: u8) -> TimerN {
        assert!(n <= self.timers);
        TimerN {
            hpet: self,
            offs: 0x100 + 0x20 * n as u64,
        }
    }

    //

    pub fn caps(&mut self) -> GeneralCaps {
        GeneralCaps(self.read_reg(0x000))
    }

    pub fn config(&mut self) -> GeneralConfig {
        GeneralConfig(self.read_reg(0x010))
    }

    pub fn set_config(&mut self, config: GeneralConfig) {
        self.write_reg(0x010, config.0)
    }

    pub fn interrupt_status(&mut self) -> GeneralInterruptStatus {
        GeneralInterruptStatus(self.read_reg(0x020))
    }

    pub fn set_interrupt_status(&mut self, status: GeneralInterruptStatus) {
        self.write_reg(0x020, status.0)
    }

    pub fn main_counter_value(&mut self) -> CounterValue {
        self.read_reg(0x0F0)
    }

    pub fn set_main_counter_value(&mut self, val: CounterValue) {
        self.write_reg(0x0F0, val)
    }

    //

    pub fn femtos(&mut self) -> u128 {
        self.period as u128 * self.main_counter_value() as u128
    }

    pub fn picos(&mut self) -> u128 {
        self.femtos() / 1_000
    }

    pub fn nanos(&mut self) -> u128 {
        self.picos() / 1_000
    }

    pub fn micros(&mut self) -> u128 {
        self.nanos() / 1_000
    }

    pub fn millis(&mut self) -> u128 {
        self.micros() / 1_000
    }

    pub fn seconds(&mut self) -> u128 {
        self.millis() / 1_000
    }

    pub fn minutes(&mut self) -> u128 {
        self.millis() / 60
    }

    pub fn now_bytes(&mut self) -> [u8; 16] {
        self.femtos().to_le_bytes()
    }

    //

    fn read_reg(&mut self, reg: u64) -> u64 {
        unsafe { read_volatile((self.addr + reg) as *const u64) }
    }

    fn write_reg(&mut self, reg: u64, val: u64) {
        unsafe { write_volatile((self.addr + reg) as *mut u64, val) }
    }

    fn init_self(&mut self) {
        let caps = self.caps();
        self.period = caps.period() as u32;
        self.timers = caps.num_tim_cap() as u8;

        // enable cnf => enable hpet
        let mut config = self.config();
        config.set_enable_cnf(1);
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

impl TimerN<'_> {
    pub fn sleep(&mut self, dur: Duration) {
        dur.num_nanoseconds();
    }

    //

    pub fn config_and_caps(&mut self) -> TimerNConfigAndCaps {
        TimerNConfigAndCaps(self.hpet.read_reg(self.offs))
    }

    pub fn set_config_and_caps(&mut self, val: TimerNConfigAndCaps) {
        self.hpet.write_reg(self.offs, val.0)
    }

    pub fn counter_value(&mut self) -> CounterValue {
        self.hpet.read_reg(self.offs + 0x8)
    }

    pub fn set_counter_value(&mut self, val: CounterValue) {
        self.hpet.write_reg(self.offs + 0x8, val)
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
        leg_rt_cnf: 1..2,
        enable_cnf: 0..1,
    }

    TimerNConfigAndCaps = u64 {

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
