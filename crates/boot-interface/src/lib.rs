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
    fn framebuffer(&self) -> Option<FramebufferCreateInfo>;

    fn name(&self) -> &'static str;
}

pub struct NopBootloader;

impl Bootloader for NopBootloader {
    fn framebuffer(&self) -> Option<FramebufferCreateInfo> {
        None
    }

    fn name(&self) -> &'static str {
        "none"
    }
}

//

static FOUND_BOOT: Once<&(dyn Bootloader + Send + Sync)> = Once::new();
