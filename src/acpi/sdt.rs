//! (Root/eXtended) System Descriptor Table
//!
//! https://wiki.osdev.org/RSDT

use crate::acpi::sdp::{Sdp, SDP};
use spin::Lazy;

//

pub static SDT: Lazy<Sdt> = Lazy::new(Sdt::init);

//

pub struct Sdt {}

//

impl Sdt {
    pub fn get() -> &'static Self {
        &SDT
    }

    pub fn init() -> Self {
        match SDP.pointer() {
            Sdp::RSDT(_) => todo!(),
            Sdp::XSDT(_) => todo!(),
        }

        todo!()
    }
}

//

struct SdtHeader {
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

impl SdtHeader {}
