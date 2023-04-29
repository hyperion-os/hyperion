//! Root/eXtended System Descriptor Table
//!
//! https://wiki.osdev.org/RSDT

use super::{rsdp::RSDP, RawSdtHeader, SdtError, StructUnpacker};
use crate::{debug, util::stack_str::StackStr};
use core::str::Utf8Error;
use spin::Lazy;

//

/// RSDT/XSDT
pub static RSDT: Lazy<Rsdt> = Lazy::new(Rsdt::init);

//

#[derive(Debug, Clone, Copy)]
pub struct Rsdt {
    /// pointers to System Descriptor Tables
    unpacker: StructUnpacker,

    pub extended: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum RsdtError {
    SdtHeader(SdtError),
}

//

unsafe impl Send for Rsdt {}
unsafe impl Sync for Rsdt {}

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

        let signature = if extended { *b"XSDT" } else { *b"RSDT" };

        let ptr = ptr as *const RawSdtHeader;
        let header = unsafe { &*ptr };
        debug!("RSDT {:?}", StackStr::from_utf8(header.signature),);

        header.validate(Some(StackStr::from_utf8(signature)?))?;

        let mut unpacker = unsafe { StructUnpacker::new(ptr as _, header.length as _) };
        let _: RawSdtHeader = unpacker.next(true).unwrap();

        Ok(Self { unpacker, extended })
    }

    pub fn iter(self) -> impl Iterator<Item = *const RawSdtHeader> {
        let ext: bool = self.extended;
        let mut unpacker: StructUnpacker = self.unpacker;

        core::iter::from_fn(move || {
            Some(if ext {
                unpacker.next::<u64>(true)? as _
            } else {
                unpacker.next::<u32>(true)? as _
            })
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
