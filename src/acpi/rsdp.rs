use crate::{boot, debug, util::stack_str::StackStr};
use core::{ffi::c_void, mem, ops::Deref, ptr::read_volatile, slice, str::Utf8Error};
use spin::Lazy;

//

pub static RSDP: Lazy<RsdpDescriptor> = Lazy::new(RsdpDescriptor::init);

//

#[derive(Debug, Clone, Copy)]
pub struct RsdpDescriptor {
    signature: StackStr<8>,
    oemid: StackStr<6>,
    version: AcpiVersion,
    rsdt_address: u64,
    xsdt_address: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpiVersion {
    V1,
    V2,
}

#[derive(Debug, Clone, Copy)]
pub enum RsdpDescriptorError {
    Utf8Error(Utf8Error),
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
    pub unsafe fn try_read_from(ptr: *const c_void) -> Result<Self, RsdpDescriptorError> {
        let rsdp: RawRsdpDescriptor2 = read_volatile(ptr as _);
        let rsdp = RsdpDescriptor::try_from(rsdp).unwrap();

        debug!("RSDP: {rsdp:#?} at {ptr:#018x?}");

        Ok(rsdp)
    }
}

//

// https://wiki.osdev.org/RSDP
#[derive(Debug, Clone, Copy)]
#[repr(packed)]
struct RawRsdpDescriptor {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
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
    extended_checksum: u8,
    reserved: [u8; 3],
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
                mem::size_of::<RawRsdpDescriptor>(),
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

impl TryFrom<RawRsdpDescriptor2> for RsdpDescriptor {
    type Error = RsdpDescriptorError;

    fn try_from(value: RawRsdpDescriptor2) -> Result<Self, Self::Error> {
        let signature = StackStr::from_utf8(value.signature)?;
        let oemid = StackStr::from_utf8(value.oemid)?;

        let version = match value.revision {
            0 => AcpiVersion::V1,
            2 => AcpiVersion::V2,
            _ => return Err(RsdpDescriptorError::InvalidRevision),
        };

        let is_valid = match version {
            AcpiVersion::V1 => value.checksum_valid_1(),
            AcpiVersion::V2 => value.checksum_valid_2(),
        };
        if !is_valid {
            return Err(RsdpDescriptorError::InvalidChecksum);
        }

        Ok(Self {
            signature,
            oemid,
            version,
            rsdt_address: value.rsdt_address as _,
            xsdt_address: value.xsdt_address,
        })
    }
}

impl From<Utf8Error> for RsdpDescriptorError {
    fn from(value: Utf8Error) -> Self {
        Self::Utf8Error(value)
    }
}
