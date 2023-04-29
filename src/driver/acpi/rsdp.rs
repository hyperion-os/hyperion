//! Root System Description Pointer
//!
//! https://wiki.osdev.org/RSDP
//!
//! This module finds the pointer to the Root/eXtended System Descriptor Table [`super::rsdt`]

use super::{bytes_sum_to_zero, AcpiOem, AcpiVersion};
use crate::{boot, debug, driver::acpi::read_unaligned_volatile, util::stack_str::StackStr};
use core::{mem, ops::Deref, str::Utf8Error};
use spin::Lazy;

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
        let rsdp: RawRsdpDescriptor = unsafe { read_unaligned_volatile(boot::rsdp() as _) };

        if rsdp.signature != *b"RSD PTR " {
            return Err(RsdpError::InvalidSignature);
        }

        let oem: AcpiOem = StackStr::from_utf8(rsdp.oem_id)?.into();
        debug!("Oem: {oem:?}");

        let version: AcpiVersion = rsdp.revision.try_into()?;

        match version {
            AcpiVersion::V1 => Self::init_rsdp(rsdp),
            AcpiVersion::V2 => Self::init_xsdp(boot::rsdp() as _),
        }
    }

    fn init_rsdp(rsdp: RawRsdpDescriptor) -> Result<Self, RsdpError> {
        let valid = unsafe { bytes_sum_to_zero(&rsdp, Some(mem::size_of::<RawRsdpDescriptor>())) };
        if !valid {
            return Err(RsdpError::InvalidChecksum);
        }

        Ok(Self {
            ptr: rsdp.rsdt_address as _,
            extended: false,
        })
    }

    fn init_xsdp(xsdp: *const RawRsdpDescriptorExt) -> Result<Self, RsdpError> {
        let xsdp = unsafe { read_unaligned_volatile(xsdp) };

        let valid = unsafe {
            bytes_sum_to_zero(
                &xsdp,
                Some(mem::size_of::<RawRsdpDescriptorExt>().min(xsdp.length as _)),
            )
        };
        if !valid {
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
    first: RawRsdpDescriptor,

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

impl Deref for RawRsdpDescriptorExt {
    type Target = RawRsdpDescriptor;

    fn deref(&self) -> &Self::Target {
        &self.first
    }
}
