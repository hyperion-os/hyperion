#![no_std]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

//

extern crate alloc;

use core::{fmt, mem, ptr, slice, str::Utf8Error};

use hyperion_driver_pic::PICS;
use hyperion_static_str::StaticStr;

//

pub mod apic;
pub mod hpet;
pub mod ioapic;
pub mod madt;
pub mod rsdp;
pub mod rsdt;

//

pub fn init() {
    PICS.lock().disable();

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
    Other(StaticStr<6>),
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
    signature: StaticStr<4>,
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: StaticStr<6>,
    oem_table_id: StaticStr<8>,
    oem_revision: u32,
    creator_id: StaticStr<4>,
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

impl From<StaticStr<6>> for AcpiOem {
    fn from(v: StaticStr<6>) -> Self {
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
            end: unsafe { first.add(bytes) },
        }
    }

    /// # Safety
    ///
    /// bytes from `first` to `first + core::mem::size_of::<T>()` must be readable
    pub const unsafe fn from<T: Sized>(first: *const T) -> Self {
        unsafe { Self::new(first as _, mem::size_of::<T>()) }
    }

    /// # Safety
    ///
    /// bytes from `first` to `end + bytes` must be readable
    pub unsafe fn extend(&mut self, bytes: usize) {
        self.end = unsafe { self.end.add(bytes) };
    }

    pub fn next<T: Copy>(&mut self, inc: bool) -> Option<T>
    where
        [(); mem::size_of::<T>()]:,
    {
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
    pub unsafe fn next_unchecked<T: Copy>(&mut self, inc: bool) -> T
    where
        [(); mem::size_of::<T>()]:,
    {
        let item = unsafe { read_unaligned_volatile(self.next as _) };

        if inc {
            self.skip(mem::size_of::<T>());
        }

        item
    }

    pub fn unpack<T: Copy>(&mut self, inc: bool) -> Result<T, SdtError>
    where
        [(); mem::size_of::<T>()]:,
    {
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
pub unsafe fn read_unaligned_volatile<T: Sized + Copy>(src: *const T) -> T
where
    [(); mem::size_of::<T>()]:,
{
    let bytes: [u8; mem::size_of::<T>()] = unsafe { ptr::read_volatile(src as *const _) };
    unsafe { ptr::read_unaligned(&bytes as *const _ as *const T) }
}

//

#[repr(C)]
pub struct ReadOnly<T = u32> {
    val: T,
    _pad: [T; 3],
}

#[repr(C)]
pub struct ReadWrite<T = u32> {
    val: T,
    _pad: [T; 3],
}

#[repr(C)]
pub struct WriteOnly<T = u32> {
    val: T,
    _pad: [T; 3],
}

#[repr(C)]
pub struct Reserved<T = u32> {
    val: T,
    _pad: [T; 3],
}

//

// TODO: should be <T: Copy> but it breaks rust-analyzer
impl<T> ReadOnly<T> {
    pub fn read(&self) -> T {
        unsafe { ptr::read_volatile(&self.val as _) }
    }
}

// TODO: should be <T: Copy> but it breaks rust-analyzer
impl<T> ReadWrite<T> {
    pub fn read(&self) -> T {
        unsafe { ptr::read_volatile(&self.val as _) }
    }

    pub fn write(&mut self, val: T) {
        unsafe { ptr::write_volatile(&mut self.val as _, val) }
    }
}

// TODO: should be <T: Copy> but it breaks rust-analyzer
impl<T> WriteOnly<T> {
    pub fn write(&mut self, val: T) {
        unsafe { ptr::write_volatile(&mut self.val as _, val) }
    }
}

impl<T: fmt::Debug + Copy> fmt::Debug for ReadOnly<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.read(), f)
    }
}

impl<T: fmt::Debug + Copy> fmt::Debug for ReadWrite<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.read(), f)
    }
}

impl<T: fmt::Debug + Copy> fmt::Debug for WriteOnly<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt("<NO READS>", f)
    }
}

impl<T: fmt::Debug + Copy> fmt::Debug for Reserved<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt("<NO READS>", f)
    }
}

/* pub trait RegRead<T> {
    fn read(&self) -> T;
}

pub trait RegWrite<T> {
    fn write(&mut self, val: T);
}

//

impl<T: Copy> RegRead<T> for Reg<ReadOnly, T> {
    fn read(&self) -> T {
        unsafe { ptr::read_volatile(&self.val as _) }
    }
}

impl<T: Copy> RegRead<T> for Reg<ReadWrite, T> {
    fn read(&self) -> T {
        unsafe { ptr::read_volatile(&self.val as _) }
    }
}

impl<T: Copy> RegWrite<T> for Reg<ReadWrite, T> {
    fn write(&mut self, val: T) {
        unsafe { ptr::write_volatile(&mut self.val as _, val) }
    }
}

impl<T: Copy> RegWrite<T> for Reg<WriteOnly, T> {
    fn write(&mut self, val: T) {
        unsafe { ptr::write_volatile(&mut self.val as _, val) }
    }
}

impl<T: fmt::Debug + Copy> fmt::Debug for Reg<ReadOnly, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.read(), f)
    }
}

impl<T: fmt::Debug + Copy> fmt::Debug for Reg<ReadWrite, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.read(), f)
    }
}

impl<T: fmt::Debug + Copy> fmt::Debug for Reg<WriteOnly, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt("<NO READS>", f)
    }
}

impl<T: fmt::Debug + Copy> fmt::Debug for Reg<(), T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt("<NO READS>", f)
    }
} */
