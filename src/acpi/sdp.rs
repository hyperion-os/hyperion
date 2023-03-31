//! (Root) System Description Pointer
//!
//! https://wiki.osdev.org/RSDP
//!
//! https://fi.wikipedia.org/wiki/Suomen_Sosialidemokraattinen_Puolue
//!
//! This module finds the pointer to the (Root/eXtended) System Descriptor Table [`super::sdt`]

use crate::{boot, debug, util::stack_str::StackStr};
use core::{mem, ops::Deref, ptr::read_volatile, slice, str::Utf8Error};
use spin::Lazy;

//

pub static SDP: Lazy<SdpDescriptor> = Lazy::new(SdpDescriptor::init);

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SdpDescriptor {
    oem: AcpiOem,
    version: AcpiVersion,
    pointer: Sdp,
}

/// System Description Pointer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sdp {
    /// ptr to Root System Descriptor Table
    RSDT(usize),

    /// ptr to eXtended System Descriptor Table
    XSDT(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpiOem {
    Bochs,
    Other(StackStr<6>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AcpiVersion {
    V1 = 1,
    V2 = 2,
}

#[derive(Debug, Clone, Copy)]
pub enum SdpDescriptorError {
    Utf8Error(Utf8Error),
    InvalidSignature,
    InvalidRevision,
    InvalidChecksum,
}

//

impl SdpDescriptor {
    pub fn get() -> &'static Self {
        &SDP
    }

    pub fn init() -> Self {
        boot::sdp()
    }

    /// # Safety
    ///
    /// * `ptr` must be [valid] for reads.
    pub unsafe fn try_read_from(ptr: *const ()) -> Result<Self, SdpDescriptorError> {
        let rsdp: RawRsdpDescriptor2 = read_volatile(ptr as _);
        let sdp = SdpDescriptor::try_from(rsdp)?;

        Ok(sdp)
    }

    pub fn pointer(&self) -> Sdp {
        self.pointer
    }
}

impl From<StackStr<6>> for AcpiOem {
    fn from(v: StackStr<6>) -> Self {
        match v.as_str() {
            "BOCHS " => Self::Bochs,
            _ => Self::Other(v),
        }
    }
}

impl AcpiVersion {
    fn checksum_valid(self, value: RawRsdpDescriptor2) -> bool {
        let length = match self {
            AcpiVersion::V1 => mem::size_of::<RawRsdpDescriptor>(),
            AcpiVersion::V2 => mem::size_of::<RawRsdpDescriptor2>().min(value.length as _),
        };
    }
}

impl TryFrom<u8> for AcpiVersion {
    type Error = SdpDescriptorError;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(AcpiVersion::V1),
            2 => Ok(AcpiVersion::V2),
            _ => Err(SdpDescriptorError::InvalidRevision),
        }
    }
}

impl TryFrom<RawRsdpDescriptor2> for SdpDescriptor {
    type Error = SdpDescriptorError;

    fn try_from(value: RawRsdpDescriptor2) -> Result<Self, Self::Error> {
        let signature = StackStr::from_utf8(value.signature)?;
        if signature.as_str() != "RSD PTR " {
            return Err(SdpDescriptorError::InvalidSignature);
        }

        let oem: AcpiOem = StackStr::from_utf8(value.oem_id)?.into();
        debug!("RSDP Oem: {oem:?}");

        let version: AcpiVersion = value.revision.try_into()?;
        if !version.checksum_valid(value) {
            return Err(SdpDescriptorError::InvalidChecksum);
        }

        let pointer = if version == AcpiVersion::V2 {
            // just reading uninitialized mem is UB af
            Sdp::XSDT(value.xsdt_address as _)
        } else {
            Sdp::RSDT(value.rsdt_address as _)
        };

        Ok(Self {
            oem,
            version,
            pointer,
        })
    }
}

impl From<Utf8Error> for SdpDescriptorError {
    fn from(value: Utf8Error) -> Self {
        Self::Utf8Error(value)
    }
}

//

// https://wiki.osdev.org/RSDP
#[derive(Debug, Clone, Copy)]
#[repr(packed)]
struct RawRsdpDescriptor {
    signature: [u8; 8],
    _checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

// https://wiki.osdev.org/RSDP
#[derive(Debug, Clone, Copy)]
#[repr(packed)]
struct RawRsdpDescriptor2 {
    first: RawRsdpDescriptor,

    length: u32,
    xsdt_address: u64,
    _extended_checksum: u8,
    _reserved: [u8; 3],
}

//

impl RawRsdpDescriptor {
    pub fn checksum_valid_1(&self) -> bool {
        let bytes: &[u8] = unsafe {
            slice::from_raw_parts(
                self as *const Self as *const u8,
                mem::size_of::<RawRsdpDescriptor>(),
            )
        };

        bytes.iter().fold(0u8, |acc, v| acc.overflowing_add(*v).0) == 0
    }
}

impl RawRsdpDescriptor2 {
    pub fn checksum_valid_2(&self) -> bool {
        let bytes: &[u8] = unsafe {
            slice::from_raw_parts(
                self as *const Self as *const u8,
                mem::size_of::<RawRsdpDescriptor>() - 3, // reserved fields d
            )
        };

        bytes.iter().fold(0u8, |acc, v| acc.overflowing_add(*v).0) == 0
    }
}

impl Deref for RawRsdpDescriptor2 {
    type Target = RawRsdpDescriptor;

    fn deref(&self) -> &Self::Target {
        &self.first
    }
}
