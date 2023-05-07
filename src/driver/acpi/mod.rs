use crate::driver::pic::PICS;
use crate::{debug, util::stack_str::StackStr};
use core::{fmt, marker::PhantomData, mem, ptr, slice, str::Utf8Error};

//

pub use madt::IO_APIC;
pub use madt::LOCAL_APIC;

//

pub mod apic;
pub mod hpet;
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

/// bitwise checksum:
///  - sum of every byte in the value
///  - does not travel recurse on ptrs/refs
pub fn checksum_of<T>(value: &T) -> u8 {
    let bytes: &[u8] =
        unsafe { slice::from_raw_parts(value as *const T as *const u8, mem::size_of::<T>()) };
    bytes.iter().fold(0u8, |acc, v| acc.wrapping_add(*v))
}

/// bitwise checksum:
///  - sum of every byte in the value
///  - does not travel recurse on ptrs/refs
pub fn checksum_of_slice<T>(value: &[T]) -> u8 {
    value
        .iter()
        .fold(0u8, |acc, v| acc.wrapping_add(checksum_of(v)))
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
    signature: StackStr<4>,
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: StackStr<6>,
    oem_table_id: StackStr<8>,
    oem_revision: u32,
    creator_id: StackStr<4>,
    creator_revision: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdtError {
    Utf8Error(Utf8Error),
    InvalidSignature,
    InvalidRevision(u8),
    InvalidChecksum,
    InvalidStructure,
}

#[derive(Debug, Clone, Copy)]
pub struct StructUnpacker {
    next: *const u8,
    end: *const u8,
}

//

impl RawSdtHeader {
    pub fn parse(
        unpacker: &mut StructUnpacker,
        signature: Option<[u8; 4]>,
    ) -> Result<RawSdtHeader, SdtError> {
        let checksum_first = unpacker.now_at();
        let header: RawSdtHeader = unpacker.next(true).ok_or(SdtError::InvalidStructure)?;

        header.oem_id.as_str_checked()?;
        header.oem_table_id.as_str_checked()?;
        header.creator_id.as_str_checked()?;

        if signature
            .map(|signature| signature != header.signature.as_bytes())
            .unwrap_or(false)
        {
            return Err(SdtError::InvalidSignature);
        }

        // header + extra
        let all_bytes = unsafe { slice::from_raw_parts(checksum_first, header.length as _) };
        let checksum = checksum_of_slice(all_bytes);

        unsafe { unpacker.extend(header.length as usize - mem::size_of::<Self>()) };

        if checksum != 0 {
            return Err(SdtError::InvalidChecksum);
        }

        Ok(header)
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
    /// bytes from `first` to `first + core::mem::size_of::<T>()` must be readable
    pub const unsafe fn from<T: Sized>(first: *const T) -> Self {
        Self::new(first as _, mem::size_of::<T>())
    }

    /// # Safety
    ///
    /// bytes from `first` to `end + bytes` must be readable
    pub unsafe fn extend(&mut self, bytes: usize) {
        self.end = self.end.add(bytes);
    }

    pub fn next<T: Copy>(&mut self, inc: bool) -> Option<T> {
        let end = unsafe { self.next.add(mem::size_of::<T>()) };

        if end > self.end {
            return None;
        }

        let item = unsafe { read_unaligned_volatile(self.next as _) };

        if inc {
            self.skip(mem::size_of::<T>());
        }

        Some(item)
    }

    /// # Safety
    ///
    /// data at `self.next` must be readable
    pub unsafe fn next_unchecked<T: Copy>(&mut self, inc: bool) -> T {
        let item = unsafe { read_unaligned_volatile(self.next as _) };

        if inc {
            self.skip(mem::size_of::<T>());
        }

        item
    }

    pub fn unpack<T: Copy>(&mut self, inc: bool) -> Result<T, SdtError> {
        self.next(inc).ok_or(SdtError::InvalidStructure)
    }

    pub fn skip(&mut self, n: usize) {
        self.next = unsafe { self.next.add(n) };
    }

    pub fn backtrack(&mut self, n: usize) {
        self.next = unsafe { self.next.sub(n) };
    }

    pub fn now_at(&self) -> *const u8 {
        self.next
    }

    pub fn left(&self) -> usize {
        (self.end as usize).saturating_sub(self.next as usize)
    }
}

/// # Safety
/// data in `ptr` must be readable (doesn't have to be aligned)
pub unsafe fn read_unaligned_volatile<T: Copy>(ptr: *const T) -> T {
    // TODO: replace this with _something_ when _something_ gets stabilized
    core::intrinsics::unaligned_volatile_load(ptr)
}

//

#[repr(C)]
pub struct Reg<const PAD: usize = 3, A = (), T = u32> {
    val: T,
    _pad: [T; PAD],
    _p: PhantomData<A>,
}

pub struct ReadOnly;
pub struct ReadWrite;
pub struct WriteOnly;

//

impl<const PAD: usize, T: Copy> Reg<PAD, ReadOnly, T> {
    pub fn read(&self) -> T {
        unsafe { ptr::read_volatile(&self.val as _) }
    }
}

impl<const PAD: usize, T: Copy> Reg<PAD, ReadWrite, T> {
    pub fn read(&self) -> T {
        unsafe { ptr::read_volatile(&self.val as _) }
    }

    pub fn write(&mut self, val: T) {
        unsafe { ptr::write_volatile(&mut self.val as _, val) }
    }
}

impl<const PAD: usize, T: Copy> Reg<PAD, WriteOnly, T> {
    pub fn write(&mut self, val: T) {
        unsafe { ptr::write_volatile(&mut self.val as _, val) }
    }
}

impl<const PAD: usize, T: fmt::Debug + Copy> fmt::Debug for Reg<PAD, ReadOnly, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.read(), f)
    }
}

impl<const PAD: usize, T: fmt::Debug + Copy> fmt::Debug for Reg<PAD, ReadWrite, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.read(), f)
    }
}

impl<const PAD: usize, T: fmt::Debug + Copy> fmt::Debug for Reg<PAD, WriteOnly, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt("<NO READS>", f)
    }
}

impl<const PAD: usize, T: fmt::Debug + Copy> fmt::Debug for Reg<PAD, (), T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt("<NO READS>", f)
    }
}
