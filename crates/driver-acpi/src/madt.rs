//! Multiple APIC Descriptor Table
//!
//! https://wiki.osdev.org/MADT

use alloc::{vec, vec::Vec};
use core::mem;

use hyperion_log::{trace, warn};
use spin::Lazy;

use super::{ioapic::IoApicInfo, rsdt::RSDT, SdtError};

//

pub static MADT: Lazy<Madt> = Lazy::new(Madt::init);

//

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Madt {
    pub local_apic_addr: usize,
    pub io_apics: Vec<IoApicInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MadtError {
    Sdt(SdtError),
    DoesntExist,
}

//

impl Madt {
    pub fn init() -> Self {
        Self::try_init().expect("MADT should be valid")
    }

    pub fn try_init() -> Result<Self, MadtError> {
        let (_, mut unpacker) = RSDT.find_table(*b"APIC").ok_or(MadtError::DoesntExist)?;
        let u = &mut unpacker;

        // skip MADT header
        let madt: RawMadt = u.unpack(true)?;
        // trace!("{madt:#x?}");

        let mut local_apic_addr = madt.local_apic_addr as usize;
        let mut io_apics = vec![];

        while let Ok(header) = u.unpack::<RawEntryHeader>(true) {
            // trace!("MADT Entry {header:?}");

            let len = header.record_len as usize;
            let data_len = len - mem::size_of::<RawEntryHeader>();

            match header.entry_type {
                0 => {
                    assert_eq!(data_len, mem::size_of::<ProcessorLocalApic>());
                    let _data: ProcessorLocalApic = u.unpack(false)?;
                    // trace!("{data:#x?}");
                }
                1 => {
                    assert_eq!(data_len, mem::size_of::<IoApic>());
                    let data: IoApic = u.unpack(false)?;
                    // trace!("{data:#x?}");

                    io_apics.push(IoApicInfo {
                        addr: data.io_apic_addr,
                        id: data.io_apic_id,
                        gsi_base: data.global_system_interrupt_base,
                    });
                }
                2 => {
                    assert_eq!(data_len, mem::size_of::<InterruptSourceOverride>());
                    let _data: InterruptSourceOverride = u.unpack(false)?;
                    // trace!("{data:#x?}");
                }
                3 => {
                    assert_eq!(data_len, mem::size_of::<NonMaskableInterruptSource>());
                    let _data: NonMaskableInterruptSource = u.unpack(false)?;
                    // trace!("{data:#x?}");
                }
                4 => {
                    assert_eq!(data_len, mem::size_of::<LocalApicNonMaskableInterrupts>());
                    let _data: LocalApicNonMaskableInterrupts = u.unpack(false)?;
                    // trace!("{data:#x?}");
                }
                5 => {
                    assert_eq!(data_len, mem::size_of::<LocalApicAddressOverride>());
                    let data: LocalApicAddressOverride = u.unpack(false)?;
                    trace!("{data:#x?}");

                    local_apic_addr = data.local_apic_addr as _;
                }
                9 => {
                    assert_eq!(data_len, mem::size_of::<ProcessorLocalx2Apic>());
                    let _data: ProcessorLocalx2Apic = u.unpack(false)?;
                    // trace!("{data:#x?}");
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
            io_apics,
        })
    }
}

impl From<SdtError> for MadtError {
    fn from(value: SdtError) -> Self {
        Self::Sdt(value)
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
