//! Root/eXtended System Descriptor Table
//!
//! https://wiki.osdev.org/RSDT

use super::{rsdp::RSDP, RawSdtHeader, SdtError};
use crate::{debug, util::stack_str::StackStr};
use core::{
    mem,
    ptr::{read_unaligned, read_volatile},
    str::Utf8Error,
};
use spin::Lazy;

//

/// RSDT/XSDT
pub static RSDT: Lazy<Rsdt> = Lazy::new(Rsdt::init);

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rsdt {
    /// pointers to System Descriptor Tables
    pub first: usize,
    /// the number of pointers
    pub len: usize,

    pub extended: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum RsdtError {
    SdtHeader(SdtError),
}

//

impl Rsdt {
    pub fn get() -> &'static Self {
        &RSDT
    }

    pub fn init() -> Self {
        Self::try_init().expect("RSDT should be valid")
    }

    pub fn try_init() -> Result<Self, RsdtError> {
        let ptr = RSDP.ptr;
        let extended = RSDP.extended;

        let (size, signature) = if extended {
            (8, *b"XSDT")
        } else {
            (4, *b"RSDT")
        };

        let ptr = ptr as *const RawSdtHeader;
        let header = unsafe { &*ptr };
        debug!("RSDT {:?}", StackStr::from_utf8(header.signature));

        header.validate(Some(StackStr::from_utf8(signature)?))?;

        Ok(Self {
            first: ptr as usize + mem::size_of::<RawSdtHeader>(),
            len: (header.length as usize - mem::size_of::<RawSdtHeader>()) / size,
            extended,
        })
    }

    pub fn iter(self) -> impl Iterator<Item = *const RawSdtHeader> {
        let first = self.first as *const u32;
        let ext = self.extended;

        (0..self.len as isize).map(move |i| {
            macro_rules! read_next_entry {
                ($t:ty) => {
                    // calculate the ptr in the SDT structure
                    //
                    // the pointer is in an array right after the RawSdtHeader without any
                    // alignment
                    let sdt_pointer_pointer = unsafe { first.offset(i) };
                    // read it from volatile memory to avoid breaking optimizations
                    let sdt_pointer_data: [u8; mem::size_of::<$t>()] =
                        unsafe { read_volatile(sdt_pointer_pointer as *const _) };
                    // and copy the potentially unaligned data to aligned data
                    (unsafe { read_unaligned::<$t>(&sdt_pointer_data as *const u8 as _) }) as _
                };
            }

            if ext {
                read_next_entry! { u64 }
            } else {
                read_next_entry! { u32 }
            }
        })
    }

    pub fn iter_headers(self) -> impl Iterator<Item = &'static RawSdtHeader> {
        self.iter().map(|ptr| {
            let header = unsafe { &*ptr };
            debug!("SDT {:?}", StackStr::from_utf8(header.signature));
            header
        })
    }
}

impl From<SdtError> for RsdtError {
    fn from(value: SdtError) -> Self {
        Self::SdtHeader(value)
    }
}

impl From<Utf8Error> for RsdtError {
    fn from(value: Utf8Error) -> Self {
        Self::SdtHeader(SdtError::Utf8Error(value))
    }
}

//

/// https://wiki.osdev.org/MADT
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(packed, C)]
struct RawMadt {
    header: RawSdtHeader,
    local_apic_address: u32,
    flags: u32,
}
