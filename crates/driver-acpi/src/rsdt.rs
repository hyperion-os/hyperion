//! Root/eXtended System Descriptor Table
//!
//! <https://wiki.osdev.org/RSDT>

use core::{
    str::Utf8Error,
    sync::atomic::{AtomicBool, Ordering},
};

use hyperion_log::debug;
use hyperion_mem::to_higher_half;
use spin::Lazy;
use x86_64::PhysAddr;

use super::{rsdp::RSDP, RawSdtHeader, SdtError, StructUnpacker};

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
        let ptr = to_higher_half(PhysAddr::new(RSDP.ptr as _));
        let extended = RSDP.extended;

        let mut unpacker = unsafe { StructUnpacker::from(ptr.as_ptr::<RawSdtHeader>()) };

        let expected_signature = if extended { *b"XSDT" } else { *b"RSDT" };
        RawSdtHeader::parse(&mut unpacker, Some(expected_signature))?;

        Ok(Self { unpacker, extended })
    }

    pub fn iter(self) -> impl Iterator<Item = StructUnpacker> {
        let ext: bool = self.extended;
        let mut unpacker: StructUnpacker = self.unpacker;

        core::iter::from_fn(move || {
            let addr = if ext {
                unpacker.next::<u64>(true)?
            } else {
                unpacker.next::<u32>(true)? as u64
            };

            Some(to_higher_half(PhysAddr::new(addr)).as_ptr::<RawSdtHeader>())
        })
        .map(|ptr| unsafe { StructUnpacker::from(ptr) })
    }

    pub fn iter_headers(self) -> impl Iterator<Item = (RawSdtHeader, StructUnpacker)> {
        static FIRST: AtomicBool = AtomicBool::new(true);
        if FIRST.swap(false, Ordering::SeqCst) {
            // On SeaBIOS QEmu:
            // FACP, APIC, HPET, MCFG, WAET
            // On OVMF QEmu:
            // FACP, APIC, HPET, MCFG, WAET, BGRT
            debug!("RSDT entries:");
            for (header, _) in self.iter_headers() {
                debug!(" - {:?}", header.signature);
            }
        }

        self.iter().filter_map(|mut unpacker| {
            let header = RawSdtHeader::parse(&mut unpacker, None).ok()?;
            Some((header, unpacker))
        })
    }

    pub fn find_table(self, signature: [u8; 4]) -> Option<(RawSdtHeader, StructUnpacker)> {
        RSDT.iter_headers()
            .find(|(header, _)| header.signature.as_bytes() == signature)
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
