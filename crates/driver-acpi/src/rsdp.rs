//! Root System Description Pointer
//!
//! https://wiki.osdev.org/RSDP
//!
//! This module finds the pointer to the Root/eXtended System Descriptor Table [`super::rsdt`]

use core::str::Utf8Error;

use hyperion_boot_interface::boot;
use hyperion_log::debug;
use hyperion_static_str::StaticStr;
use spin::Lazy;

use super::{checksum_of, AcpiOem, AcpiVersion};
use crate::StructUnpacker;

//

pub static RSDP: Lazy<Rsdp> = Lazy::new(Rsdp::init);

//

/// Root/eXtended System Description Pointer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rsdp {
    /// pointer to Root/eXtended System Descriptor Table
    pub ptr: usize,
    /// `ptr` is XSDT pointer instead of RSDT pointer
    pub extended: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum RsdpError {
    Utf8Error(Utf8Error),
    InvalidSignature,
    InvalidRevision(u8),
    InvalidChecksum,
    NoRsdp,
}

//

impl Rsdp {
    pub fn get() -> Self {
        *RSDP
    }

    pub fn init() -> Self {
        Self::try_init().expect("Failed to read RSDP")
    }

    pub fn try_init() -> Result<Self, RsdpError> {
        let rsdp = boot().rsdp().ok_or(RsdpError::NoRsdp)?;

        let mut unpacker = unsafe { StructUnpacker::from(rsdp as *const RawRsdpDescriptor) };

        let rsdp: RawRsdpDescriptor = unsafe { unpacker.next_unchecked(true) };

        if rsdp.signature != *b"RSD PTR " {
            return Err(RsdpError::InvalidSignature);
        }

        let oem: AcpiOem = StaticStr::from_utf8(rsdp.oem_id)?.into();
        debug!("ACPI Oem: {oem:?}");

        let version: AcpiVersion = rsdp.revision.try_into()?;
        debug!("ACPI Version: {version:?}");

        match version {
            AcpiVersion::V1 => Self::init_rsdp(rsdp),
            AcpiVersion::V2 => Self::init_xsdp(rsdp, &mut unpacker),
        }
    }

    fn init_rsdp(rsdp: RawRsdpDescriptor) -> Result<Self, RsdpError> {
        debug!("System descriptor pointer is RSDP");

        if checksum_of(&rsdp) != 0 {
            return Err(RsdpError::InvalidChecksum);
        }

        Ok(Self {
            ptr: rsdp.rsdt_address as _,
            extended: false,
        })
    }

    fn init_xsdp(
        rsdp: RawRsdpDescriptor,
        unpacker: &mut StructUnpacker,
    ) -> Result<Self, RsdpError> {
        debug!("System descriptor pointer is XSDP (eXtended)");

        let xsdp: RawRsdpDescriptorExt = unsafe { unpacker.next_unchecked(true) };

        if checksum_of(&(rsdp, xsdp)) != 0 {
            return Err(RsdpError::InvalidChecksum);
        }

        Ok(Self {
            ptr: xsdp.xsdt_address as _,
            extended: true,
        })
    }
}

impl From<Utf8Error> for RsdpError {
    fn from(value: Utf8Error) -> Self {
        Self::Utf8Error(value)
    }
}

//

// https://wiki.osdev.org/RSDP
#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct RawRsdpDescriptor {
    signature: [u8; 8],
    _checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

// https://wiki.osdev.org/RSDP
#[derive(Debug, Clone, Copy)]
#[repr(packed, C)]
struct RawRsdpDescriptorExt {
    length: u32,
    xsdt_address: u64,
    _extended_checksum: u8,
    _reserved: [u8; 3],
}

//

impl TryFrom<u8> for AcpiVersion {
    type Error = RsdpError;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(AcpiVersion::V1),
            2 => Ok(AcpiVersion::V2),
            _ => Err(RsdpError::InvalidRevision(v)),
        }
    }
}
