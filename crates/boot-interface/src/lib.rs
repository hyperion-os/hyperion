#![no_std]

//

use spin::Once;

//

pub fn boot() -> AnyBootloader {
    static NOP_BOOT: NopBootloader = NopBootloader;
    FOUND_BOOT.get().copied().unwrap_or(&NOP_BOOT)
}

pub fn provide_boot(boot: AnyBootloader) -> AnyBootloader {
    *FOUND_BOOT.call_once(|| boot)
}

//

pub type AnyBootloader = &'static (dyn Bootloader + Send + Sync + 'static);

pub struct FramebufferCreateInfo {
    pub buf: &'static mut [u8],
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
}

pub trait Bootloader {
    fn name(&self) -> &'static str;

    fn framebuffer(&self) -> Option<FramebufferCreateInfo>;

    /// higher half direct map offset
    fn hhdm_offset(&self) -> u64;

    /// root system descriptor pointer
    fn rsdp(&self) -> Option<*const ()>;
}

pub struct NopBootloader;

impl Bootloader for NopBootloader {
    fn name(&self) -> &'static str {
        "none"
    }

    fn framebuffer(&self) -> Option<FramebufferCreateInfo> {
        None
    }

    fn hhdm_offset(&self) -> u64 {
        todo!()
    }

    fn rsdp(&self) -> Option<*const ()> {
        None
    }
}

//

static FOUND_BOOT: Once<&(dyn Bootloader + Send + Sync)> = Once::new();
