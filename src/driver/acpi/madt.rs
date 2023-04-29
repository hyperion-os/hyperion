//! Multiple APIC Descriptor Table
//!
//! https://wiki.osdev.org/MADT

use super::{rsdt::RSDT, SdtError};
use crate::{trace, warn};
use core::mem;
use spin::Lazy;

//

pub static MADT: Lazy<Madt> = Lazy::new(Madt::init);
pub static LOCAL_APIC: Lazy<usize> = Lazy::new(|| MADT.local_apic_addr);
pub static IO_APIC: Lazy<Option<usize>> = Lazy::new(|| MADT.io_apic_addr);

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Madt {
    local_apic_addr: usize,
    io_apic_addr: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MadtError {
    SdtHeader(SdtError),
    DoesntExist,
}

//

impl Madt {
    pub fn get() -> Self {
        *MADT
    }

    pub fn init() -> Self {
        Self::try_init().expect("MADT should be valid")
    }

    pub fn try_init() -> Result<Self, MadtError> {
        let Some((_, mut unpacker)) = RSDT
            .iter_headers()
            .find(|(header, _)| {
                header.signature == *b"APIC"
            }) else {
                return Err(MadtError::DoesntExist);
            };
        let u = &mut unpacker;

        // skip MADT header
        let madt: RawMadt = u.unpack(true)?;
        trace!("{madt:?}");

        let mut local_apic_addr = madt.local_apic_addr as usize;
        let mut io_apic_addr = None;

        while let Ok(header) = u.unpack::<RawEntryHeader>(true) {
            // trace!("MADT Entry {header:?}");

            let len = header.record_len as usize;
            let data_len = len - mem::size_of::<RawEntryHeader>();

            match header.entry_type {
                0 => {
                    assert_eq!(data_len, mem::size_of::<ProcessorLocalApic>());
                    let data: ProcessorLocalApic = u.unpack(false)?;
                    trace!("{data:?}");
                }
                1 => {
                    assert_eq!(data_len, mem::size_of::<IoApic>());
                    let data: IoApic = u.unpack(false)?;
                    trace!("{data:?}");

                    io_apic_addr = Some(data.io_apic_addr as usize);
                }
                2 => {
                    assert_eq!(data_len, mem::size_of::<InterruptSourceOverride>());
                    let data: InterruptSourceOverride = u.unpack(false)?;
                    trace!("{data:?}");
                }
                3 => {
                    assert_eq!(data_len, mem::size_of::<NonMaskableInterruptSource>());
                    let data: NonMaskableInterruptSource = u.unpack(false)?;
                    trace!("{data:?}");
                }
                4 => {
                    assert_eq!(data_len, mem::size_of::<LocalApicNonMaskableInterrupts>());
                    let data: LocalApicNonMaskableInterrupts = u.unpack(false)?;
                    trace!("{data:?}");
                }
                5 => {
                    assert_eq!(data_len, mem::size_of::<LocalApicAddressOverride>());
                    let data: LocalApicAddressOverride = u.unpack(false)?;
                    trace!("{data:?}");

                    local_apic_addr = data.local_apic_addr as usize;
                }
                9 => {
                    assert_eq!(data_len, mem::size_of::<ProcessorLocalx2Apic>());
                    let data: ProcessorLocalx2Apic = u.unpack(false)?;
                    trace!("{data:?}");
                }
                _ => {
                    warn!("Unidentified MADT Entry");
                }
            }

            u.skip(data_len);
        }

        debug_assert_eq!(unpacker.end, unpacker.next);

        Ok(Self {
            local_apic_addr,
            io_apic_addr,
        })
    }
}

impl From<SdtError> for MadtError {
    fn from(value: SdtError) -> Self {
        Self::SdtHeader(value)
    }
}

//

#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct RawMadt {
    local_apic_addr: u32,
    flags: u32,
}

// #[allow(unused)]
#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct RawEntryHeader {
    entry_type: u8,
    record_len: u8,
}

// #[allow(unused)]
#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct ProcessorLocalApic {
    acpi_processor_id: u8,
    apic_id: u8,
    flags: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct IoApic {
    io_apic_id: u8,
    _reserved: u8,
    io_apic_addr: u32,
    global_system_interrupt_base: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct InterruptSourceOverride {
    bus_source: u8,
    irq_source: u8,
    global_system_interrupt: u32,
    flags: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct NonMaskableInterruptSource {
    nmi_source: u8,
    _reserved: u8,
    flags: u16,
    global_system_interrupt: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct LocalApicNonMaskableInterrupts {
    acpi_processor_id: u8,
    flags: u16,
    lint: u8,
}

#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct LocalApicAddressOverride {
    reserved: u16,
    local_apic_addr: u64,
}

#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct ProcessorLocalx2Apic {
    _reserved: u16,
    processor_local_x2_apic_id: u32,
    flags: u32,
    acpi_id: u32,
}

//
