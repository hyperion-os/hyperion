//! Root System Description Pointer
//!
//! https://wiki.osdev.org/RSDP
//!
//! This module finds the pointer to the Root/eXtended System Descriptor Table [`super::rsdt`]

use super::{bytes_sum_to_zero, AcpiOem, AcpiVersion};
use crate::{boot, debug, util::stack_str::StackStr};
use core::{mem, ops::Deref, ptr::read_volatile, str::Utf8Error};
use spin::Lazy;

//

pub static RSDP: Lazy<Rsdp> = Lazy::new(Rsdp::init);

//

/// Root/eXtended System Description Pointer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rsdp {
    /// pointer to Root/eXtended System Descriptor Table
    pub ptr: usize,
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
        let rsdp: RawRsdpDescriptor2 = unsafe { read_volatile(boot::rsdp() as _) };

        if rsdp.signature != *b"RSD PTR " {
            return Err(RsdpError::InvalidSignature);
        }

        let oem: AcpiOem = StackStr::from_utf8(rsdp.oem_id)?.into();
        debug!("Oem: {oem:?}");

        let version: AcpiVersion = rsdp.revision.try_into()?;
        if !version.checksum_valid(&rsdp) {
            return Err(RsdpError::InvalidChecksum);
        }

        Ok(if version == AcpiVersion::V2 {
            Self {
                ptr: rsdp.xsdt_address as _,
                extended: true,
            }
        } else {
            Self {
                ptr: rsdp.rsdt_address as _,
                extended: false,
            }
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
    type Error = RsdpError;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(AcpiVersion::V1),
            2 => Ok(AcpiVersion::V2),
            _ => Err(RsdpError::InvalidRevision(v)),
        }
    }
}

impl Deref for RawRsdpDescriptor2 {
    type Target = RawRsdpDescriptor;

    fn deref(&self) -> &Self::Target {
        &self.first
    }
}
