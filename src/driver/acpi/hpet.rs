//! High Precision Event Timer
//!
//! https://wiki.osdev.org/HPET

use core::ptr::{read_volatile, write_volatile};

use bit_field::BitField;
use spin::Lazy;

use crate::debug;

use super::{rsdt::RSDT, SdtError};

//

pub static HPET: Lazy<Hpet> = Lazy::new(Hpet::init);

//

#[derive(Debug)]
pub struct Hpet {
    addr: u64,
    // regs: Mutex<&'static mut HpetRegs>,
}

#[derive(Debug)]
pub struct HpetRegs {
    // general_caps: Reg<1, ReadOnly, GeneralCaps>,
    // general_config: Reg<1, ReadWrite, GeneralConfig>,
    // general_interrupt_status: Reg<1, ReadWrite, GeneralInterruptStatus>,
    // main_counter_value: Reg<1, ReadWrite, MainCounterValue>,
}

#[derive(Debug, Clone, Copy)]
pub enum HpetError {
    Sdt(SdtError),
    DoesntExist,
}

//

impl Hpet {
    pub fn get() -> &'static Self {
        &HPET
    }

    pub fn init() -> Self {
        Self::try_init().expect("MADT should be valid")
    }

    pub fn try_init() -> Result<Self, HpetError> {
        let (_, mut unpacker) = RSDT.find_table(*b"HPET").ok_or(HpetError::DoesntExist)?;
        let u = &mut unpacker;

        let hpet: RawHpet = u.unpack(true)?;

        debug!("HPET initialized {hpet:#x?}");

        let addr = hpet.address.address;
        debug!("HPET address {addr:#x}");

        /* let mut regs = Mutex::new(unsafe { &mut *(addr as *mut HpetRegs) });

        debug!("{:#?}", regs.get_mut());

        regs.get_mut().general_config.read(); */

        Ok(Self { addr })
    }

    pub fn general_caps(&mut self) -> GeneralCaps {
        GeneralCaps(self.read_reg(0x000))
    }

    pub fn general_config(&mut self) -> GeneralConfig {
        GeneralConfig(self.read_reg(0x010))
    }

    pub fn set_general_config(&mut self, config: GeneralConfig) {
        self.write_reg(0x010, config.0)
    }

    pub fn general_interrupt_status(&mut self) -> GeneralInterruptStatus {
        GeneralInterruptStatus(self.read_reg(0x020))
    }

    pub fn set_general_interrupt_status(&mut self, status: GeneralInterruptStatus) {
        self.write_reg(0x020, status.0)
    }

    pub fn main_counter_value(&mut self) -> MainCounterValue {
        self.read_reg(0x030)
    }

    pub fn set_main_counter_value(&mut self, val: MainCounterValue) {
        self.write_reg(0x0F0, val)
    }

    /* pub fn timer_n_config_and_caps(&mut self) -> TimerNConfigAndCaps {
        self.read_reg(0x030)
    }

    pub fn set_timer_n_config_and_caps(&mut self, val: MainCounterValue) {
        self.write_reg(0x0F0, val)
    } */

    fn read_reg(&mut self, reg: u64) -> u64 {
        unsafe { read_volatile((self.addr + reg) as *const u64) }
    }

    fn write_reg(&mut self, reg: u64, val: u64) {
        unsafe { write_volatile((self.addr + reg) as *mut u64, val) }
    }

    #[allow(unused)]
    const fn freq() -> u32 {
        const SECOND: u64 = 10u64.pow(15);
        const _100_NANOS: u32 = (SECOND / 10_000_000) as u32;
        _100_NANOS
    }
}

impl From<SdtError> for HpetError {
    fn from(value: SdtError) -> Self {
        Self::Sdt(value)
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
                    f.debug_struct("GeneralCaps")
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

pub type MainCounterValue = u64;

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
