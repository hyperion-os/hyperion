//! Root/eXtended System Descriptor Table
//!
//! https://wiki.osdev.org/RSDT

use crate::{
    acpi::rsdp::{Rsdp, RSDP},
    debug,
    util::stack_str::StackStr,
};
use core::{mem, ops::Add, ptr::read_unaligned, slice, str::Utf8Error};
use spin::Lazy;

//

/// RSDT/XSDT
pub static RSDT: Lazy<Rsdt> = Lazy::new(Rsdt::init);

//

pub struct Rsdt {}

#[derive(Debug, Clone, Copy)]
pub enum RsdtError {
    Utf8Error(Utf8Error),
    InvalidSignature,
    InvalidRevision,
    InvalidChecksum,
}

//

impl Rsdt {
    pub fn get() -> &'static Self {
        &RSDT
    }

    pub fn init() -> Self {
        match RSDP.pointer() {
            Rsdp::RSDT(ptr) => {
                let header: RawSdtHeader = unsafe { read_unaligned(ptr as *const RawSdtHeader) };

                debug!("RSDT Header {header:#?}");
                debug!(
                    "RSDT Header signature {:?}",
                    StackStr::from_utf8(header.signature)
                );
                debug!(
                    "RSDT Header oem_id {:?}",
                    StackStr::from_utf8(header.oem_id)
                );
                debug!(
                    "RSDT Header oem_table_id {:?}",
                    StackStr::from_utf8(header.oem_table_id)
                );

                let sdt_pointers = (header.length as usize - mem::size_of::<RawSdtHeader>()) / 4;
                let sdt_pointers = unsafe {
                    slice::from_raw_parts(
                        (ptr + mem::size_of::<RawSdtHeader>()) as *const u32,
                        sdt_pointers,
                    )
                };

                debug!("RSDT pointers: {sdt_pointers:?}",);
            }
            Rsdp::XSDT(_) => todo!(),
        }

        todo!()
    }
}

impl TryFrom<RawSdtHeader> for Rsdt {
    type Error = RsdtError;

    fn try_from(value: RawSdtHeader) -> Result<Self, Self::Error> {
        if value.signature != *b"RSDT" {
            return Err(RsdtError::InvalidSignature);
        }

        todo!()
    }
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(packed, C)]
struct RawSdtHeader {
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

//

impl RawSdtHeader {}

//
