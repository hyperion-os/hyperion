use crate::driver::pic::PICS;
use crate::{debug, util::stack_str::StackStr};
use core::{
    mem,
    ptr::{read_unaligned, read_volatile},
    slice,
    str::Utf8Error,
};

//

pub use madt::IO_APIC;
pub use madt::LOCAL_APIC;

//

pub mod apic;
pub mod madt;
pub mod rsdp;
pub mod rsdt;

//

pub fn init() {
    PICS.lock().disable();

    debug!("{:018x?}", *IO_APIC);
    debug!("{:018x?}", *LOCAL_APIC);

    apic::enable();
}

/// checksum_validation
///
/// sums up every byte in the structure
///
/// # Safety
///
/// * `size` has to be `None` or the memory range must be readable
unsafe fn bytes_sum_to_zero<T>(ptr: *const T, size: Option<usize>) -> bool {
    let size = size.unwrap_or(mem::size_of::<T>());
    let bytes: &[u8] = unsafe { slice::from_raw_parts(ptr as *const u8, size) };

    bytes.iter().fold(0u8, |acc, v| acc.overflowing_add(*v).0) == 0
}

//

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(packed, C)]
pub struct RawSdtHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdtError {
    Utf8Error(Utf8Error),
    InvalidSignature,
    InvalidRevision(u8),
    InvalidChecksum,
}

#[derive(Debug, Clone, Copy)]
pub struct StructUnpacker {
    next: *const u8,
    end: *const u8,
}

//

impl RawSdtHeader {
    pub fn validate(&self, signature: Option<StackStr<4>>) -> Result<(), SdtError> {
        let parsed_signature = StackStr::from_utf8(self.signature)?;
        if signature
            .map(|signature| signature != parsed_signature)
            .unwrap_or(false)
        {
            return Err(SdtError::InvalidSignature);
        }

        _ = StackStr::from_utf8(self.oem_id)?;

        _ = StackStr::from_utf8(self.oem_table_id)?;

        let is_valid =
            unsafe { bytes_sum_to_zero(self as *const Self, Some(self.length as usize)) };
        if !is_valid {
            return Err(SdtError::InvalidChecksum);
        }

        Ok(())
    }
}

impl From<Utf8Error> for SdtError {
    fn from(value: Utf8Error) -> Self {
        Self::Utf8Error(value)
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

impl StructUnpacker {
    /// # Safety
    ///
    /// bytes from `first` to `first + bytes` must be readable
    pub const unsafe fn new(first: *const u8, bytes: usize) -> Self {
        Self {
            next: first,
            end: first.add(bytes),
        }
    }

    /// # Safety
    ///
    /// bytes from `first` to `first + bytes` must be readable
    pub unsafe fn resize(&mut self, bytes: usize) {
        self.end = self.next.add(bytes);
    }

    pub fn next<T: Copy>(&mut self, inc: bool) -> Option<T> {
        let end = unsafe { self.next.add(mem::size_of::<T>()) };

        if end > self.end {
            return None;
        }

        let item = unsafe { read_unaligned_volatile(self.next as _) };

        // let bytes: [u8; SIZE] = unsafe { read_volatile(self.next as _) };
        // let item = unsafe { read_unaligned(&bytes as *const u8 as *const T) };

        if inc {
            self.skip(mem::size_of::<T>());
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

pub unsafe fn read_unaligned_volatile<T: Copy>(ptr: *const T) -> T {
    // TODO: replace this with _something_ when _something_ gets stabilized
    core::intrinsics::unaligned_volatile_load(ptr)
}
