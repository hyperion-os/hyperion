//! Multiple APIC Descriptor Table
//!
//! https://wiki.osdev.org/MADT

use super::SdtError;
use crate::{
    acpi::{rsdt::RSDT, RawSdtHeader},
    trace, warn,
};
use core::{
    mem,
    ptr::{read_unaligned, read_volatile},
};
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
    InvalidStructure,
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
        let Some(madt) = RSDT
            .iter_headers()
            .find(|header| header.signature == *b"APIC") else {
                return Err(MadtError::DoesntExist);
            };

        madt.validate(None)?;

        let mut unpacker = StructUnpacker {
            next: (madt as *const _ as usize + mem::size_of::<RawSdtHeader>()) as _,
            end: (madt as *const _ as usize + madt.length as usize) as _,
        };

        let mut local_apic_addr;
        let mut io_apic_addr = None;

        macro_rules! unpack {
            ($unpacker:expr, $t:ty) => {
                unpack!($unpacker, $t, true)
            };

            ($unpacker:expr, $t:ty, $inc:expr) => {
                $unpacker
                    .next::<{ mem::size_of::<$t>() }, $t>($inc)
                    .ok_or(MadtError::InvalidStructure)
            };
        }

        let madt = unpack!(unpacker, RawMadt)?;
        trace!("{madt:?}");

        local_apic_addr = madt.local_apic_addr as usize;

        while let Ok(header) = unpack!(unpacker, RawEntryHeader, true) {
            // trace!("MADT Entry {header:?}");

            match header.entry_type {
                0 => {
                    assert_eq!(
                        header.record_len as usize,
                        mem::size_of::<(RawEntryHeader, ProcessorLocalApic)>()
                    );
                    let data = unpack!(unpacker, ProcessorLocalApic, false)?;
                    trace!("{data:?}");
                }
                1 => {
                    assert_eq!(
                        header.record_len as usize,
                        mem::size_of::<(RawEntryHeader, IoApic)>()
                    );
                    let data = unpack!(unpacker, IoApic, false)?;
                    trace!("{data:?}");

                    io_apic_addr = Some(data.io_apic_addr as usize);
                }
                2 => {
                    assert_eq!(
                        header.record_len as usize,
                        mem::size_of::<(RawEntryHeader, InterruptSourceOverride)>()
                    );
                    let data = unpack!(unpacker, InterruptSourceOverride, false)?;
                    trace!("{data:?}");
                }
                3 => {
                    assert_eq!(
                        header.record_len as usize,
                        mem::size_of::<(RawEntryHeader, NonMaskableInterruptSource)>()
                    );
                    let data = unpack!(unpacker, NonMaskableInterruptSource, false)?;
                    trace!("{data:?}");
                }
                4 => {
                    assert_eq!(
                        header.record_len as usize,
                        mem::size_of::<(RawEntryHeader, LocalApicNonMaskableInterrupts)>()
                    );
                    let data = unpack!(unpacker, LocalApicNonMaskableInterrupts, false)?;
                    trace!("{data:?}");
                }
                5 => {
                    let data = unpack!(unpacker, LocalApicAddressOverride, false)?;
                    trace!("{data:?}");

                    local_apic_addr = data.local_apic_addr as usize;
                }
                9 => {
                    let data = unpack!(unpacker, ProcessorLocalx2Apic, false)?;
                    trace!("{data:?}");
                }
                _ => {
                    warn!("Unidentified MADT Entry");
                }
            }

            unpacker.skip(header.record_len as usize);
            unpacker.backtrack(mem::size_of::<RawEntryHeader>());
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

struct StructUnpacker {
    next: *const u8,
    end: *const u8,
}

//

impl StructUnpacker {
    pub fn next<const SIZE: usize, T: Copy>(&mut self, inc: bool) -> Option<T> {
        let end = unsafe { self.next.add(SIZE) };

        if end > self.end {
            return None;
        }

        let bytes: [u8; SIZE] = unsafe { read_volatile(self.next as _) };
        let item = unsafe { read_unaligned(&bytes as *const u8 as *const T) };

        if inc {
            self.skip(SIZE);
        }

        Some(item)
    }

    pub fn skip(&mut self, n: usize) {
        self.next = unsafe { self.next.add(n) };
    }

    pub fn backtrack(&mut self, n: usize) {
        self.next = unsafe { self.next.sub(n) };
    }
}
