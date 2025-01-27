#![no_std]
#![feature(pointer_is_aligned_to, box_into_boxed_slice)]

//

extern crate alloc;

use alloc::{boxed::Box, format, string::String};
use core::ops::Range;

use hyperion_arch::vmm::PageMap;
use hyperion_framebuffer::framebuffer::Framebuffer;
use hyperion_mem::{
    from_higher_half,
    pmm::PageFrame,
    vmm::{MapTarget, PageMapImpl},
};
use hyperion_vfs::{device::FileDevice, Error, Result};
use spin::{MutexGuard, Once};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub struct FboDevice {
    maps: usize,

    lock: Option<(MutexGuard<'static, Framebuffer>, PageFrame)>,
}

pub struct FboInfoDevice {
    info: Once<String>,
}

//

impl FileDevice for FboDevice {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn len(&self) -> usize {
        if let Some(fbo) = self.lock.as_ref() {
            fbo.0.buf().len()
        } else {
            Self::with(|fbo| fbo.len())
        }
    }

    fn map_phys(
        &mut self,
        vmm: &PageMap,
        v_addr: Range<VirtAddr>,
        flags: PageTableFlags,
    ) -> Result<usize> {
        if !v_addr.start.is_aligned(0x1000u64) {
            // FIXME: use the real abi error
            return Err(Error::PERMISSION_DENIED);
        }

        self.maps = self.maps.checked_add(1).ok_or(Error::FILESYSTEM_ERROR)?;

        let (_, frame) = self.lock.get_or_insert_with(|| {
            let fbo = Framebuffer::get().unwrap().lock();

            let start = fbo.buf().as_ptr();
            let size = fbo.buf().len();

            assert!(
                start.is_aligned_to(0x1000) && (size as *const u8).is_aligned_to(0x1000),
                "framebuffer isn't aligned to a page",
            );

            let start = from_higher_half(VirtAddr::from_ptr(start));

            (fbo, unsafe { PageFrame::new(start, size >> 12) })
        });

        let end = v_addr.end.min(v_addr.start + frame.byte_len());
        vmm.map(
            v_addr.start..end,
            MapTarget::Borrowed(frame.physical_addr()),
            flags,
        );

        hyperion_log::debug!("FBO mapped");

        Ok((end - v_addr.start) as usize)
    }

    fn unmap_phys(&mut self) -> Result<()> {
        self.maps = self.maps.checked_sub(1).ok_or(Error::FILESYSTEM_ERROR)?;

        if self.maps == 0 {
            self.lock = None;
        }

        Ok(())
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        Self::with(|fbo| fbo.read(offset, buf))
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize> {
        Self::with_mut(|fbo| fbo.write(offset, buf))
    }
}

impl FboDevice {
    pub const fn new() -> Self {
        Self {
            maps: 0,
            lock: None,
        }
    }

    pub fn with<T>(f: impl FnOnce(&[u8]) -> T) -> T {
        let fbo = Framebuffer::get();
        let mut lock;
        let this = if let Some(fbo) = fbo {
            lock = fbo.lock();
            &*lock.buf_mut()
        } else {
            &[]
        };

        f(this)
    }

    pub fn with_mut<T>(f: impl FnOnce(&mut [u8]) -> T) -> T {
        let fbo = Framebuffer::get();
        let mut lock;
        let this = if let Some(fbo) = fbo {
            lock = fbo.lock();
            lock.buf_mut()
        } else {
            &mut []
        };

        f(this)
    }
}

impl FileDevice for FboInfoDevice {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn len(&self) -> usize {
        self.get().len()
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        self.get().as_bytes().read(offset, buf)
    }
}

impl FboInfoDevice {
    pub const fn new() -> Self {
        Self { info: Once::new() }
    }

    pub fn get(&self) -> &str {
        self.info
            .try_call_once(|| {
                if let Some(fbo) = Framebuffer::get() {
                    let fbo = fbo.lock();
                    let info = format!("{}:{}:{}:{}", fbo.width, fbo.height, fbo.pitch, 32);
                    Ok(info)
                } else {
                    Err(())
                }
            })
            .map(|s| s.as_str())
            .unwrap_or("")
    }
}
