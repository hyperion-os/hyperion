#![no_std]
#![feature(pointer_is_aligned)]

//

use hyperion_framebuffer::framebuffer::Framebuffer;
use hyperion_log::{debug, error};
use hyperion_mem::from_higher_half;
use hyperion_vfs::{
    device::FileDevice,
    error::{IoError, IoResult},
};
use spin::MutexGuard;
use x86_64::VirtAddr;

//

pub struct FboDevice {
    maps: usize,

    lock: Option<MutexGuard<'static, Framebuffer>>,
}

//

impl FileDevice for FboDevice {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn len(&self) -> usize {
        if let Some(fbo) = self.lock.as_ref() {
            fbo.buf().len()
        } else {
            Self::with(|fbo| fbo.len())
        }
    }

    fn map_phys(&mut self, size_bytes: usize) -> IoResult<usize> {
        self.maps = self.maps.checked_add(1).ok_or(IoError::FilesystemError)?;

        let lock = self
            .lock
            .get_or_insert_with(|| Framebuffer::get().unwrap().lock());

        let buf = lock.buf_mut();

        let start = buf.as_mut_ptr();
        let size = buf.len();

        if size_bytes > size {
            return Err(IoError::UnexpectedEOF);
        }
        if !start.is_aligned_to(0x1000) || size % 0x1000 != 0 {
            error!("framebuffer isnt aligned to a page");
            return Err(IoError::FilesystemError);
        }

        let start = from_higher_half(VirtAddr::new(start as u64));

        Ok(start.as_u64() as _)
    }

    fn unmap_phys(&mut self) -> IoResult<()> {
        self.maps = self.maps.checked_sub(1).ok_or(IoError::FilesystemError)?;

        if self.maps == 0 {
            self.lock = None;
        }

        Ok(())
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        Self::with(|fbo| fbo.read(offset, buf))
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize> {
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
