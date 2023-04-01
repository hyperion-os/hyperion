//! Root System Description Pointer
//!
//! https://wiki.osdev.org/RSDP
//!
//! This module finds the pointer to the Root/eXtended System Descriptor Table [`super::rsdt`]

use super::{bytes_sum_to_zero, AcpiVersion};
use crate::{acpi::AcpiOem, boot, debug, util::stack_str::StackStr};
use core::{mem, ops::Deref, ptr::read_volatile, str::Utf8Error};
use spin::Lazy;

//

pub static RSDP: Lazy<RsdpDescriptor> = Lazy::new(RsdpDescriptor::init);

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RsdpDescriptor {
    oem: AcpiOem,
    version: AcpiVersion,
    pointer: Rsdp,
}

/// Root/eXtended System Description Pointer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rsdp {
    /// ptr to Root System Descriptor Table
    RSDT(usize),

    /// ptr to eXtended System Descriptor Table
    XSDT(usize),
}

#[derive(Debug, Clone, Copy)]
pub enum RsdpDescriptorError {
    Utf8Error(Utf8Error),
    InvalidSignature,
    InvalidRevision,
    InvalidChecksum,
}

//

impl RsdpDescriptor {
    pub fn get() -> &'static Self {
        &RSDP
    }

    pub fn init() -> Self {
        boot::rsdp()
    }

    /// # Safety
    ///
    /// * `ptr` must be [valid] for reads.
    pub unsafe fn try_read_from(ptr: *const ()) -> Result<Self, RsdpDescriptorError> {
        let rsdp: RawRsdpDescriptor2 = read_volatile(ptr as _);
        let sdp = RsdpDescriptor::try_from(rsdp)?;

        Ok(sdp)
    }

    pub fn pointer(&self) -> Rsdp {
        self.pointer
    }
}

impl TryFrom<RawRsdpDescriptor2> for RsdpDescriptor {
    type Error = RsdpDescriptorError;

    fn try_from(value: RawRsdpDescriptor2) -> Result<Self, Self::Error> {
        if value.signature != *b"RSD PTR " {
            return Err(RsdpDescriptorError::InvalidSignature);
        }

        let oem: AcpiOem = StackStr::from_utf8(value.oem_id)?.into();
        debug!("RSDP Oem: {oem:?}");

        let version: AcpiVersion = value.revision.try_into()?;
        if !version.checksum_valid(&value) {
            return Err(RsdpDescriptorError::InvalidChecksum);
        }

        let pointer = if version == AcpiVersion::V2 {
            Rsdp::XSDT(value.xsdt_address as _)
        } else {
            Rsdp::RSDT(value.rsdt_address as _)
        };

        Ok(Self {
            oem,
            version,
            pointer,
        })
    }
}

impl From<Utf8Error> for RsdpDescriptorError {
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
struct RawRsdpDescriptor2 {
    first: RawRsdpDescriptor,

    length: u32,
    xsdt_address: u64,
    _extended_checksum: u8,
    _reserved: [u8; 3],
}

//

impl AcpiVersion {
    fn checksum_valid(self, value: &RawRsdpDescriptor2) -> bool {
        let length = match self {
            AcpiVersion::V1 => mem::size_of::<RawRsdpDescriptor>(),
            AcpiVersion::V2 => mem::size_of::<RawRsdpDescriptor2>().min(value.length as _),
        };

        unsafe { bytes_sum_to_zero(value, Some(length)) }
    }
}

impl TryFrom<u8> for AcpiVersion {
    type Error = RsdpDescriptorError;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(AcpiVersion::V1),
            2 => Ok(AcpiVersion::V2),
            _ => Err(RsdpDescriptorError::InvalidRevision),
        }
    }
}

impl Deref for RawRsdpDescriptor2 {
    type Target = RawRsdpDescriptor;

    fn deref(&self) -> &Self::Target {
        &self.first
    }
}
