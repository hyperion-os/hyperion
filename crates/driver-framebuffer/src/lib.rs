#![no_std]
#![feature(pointer_is_aligned)]

//

extern crate alloc;

use alloc::{format, string::String};

use hyperion_framebuffer::framebuffer::Framebuffer;
use hyperion_log::error;
use hyperion_mem::from_higher_half;
use hyperion_vfs::{
    device::FileDevice,
    error::{IoError, IoResult},
};
use spin::{MutexGuard, Once};
use x86_64::VirtAddr;

//

pub struct FboDevice {
    maps: usize,

    lock: Option<MutexGuard<'static, Framebuffer>>,
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

impl FileDevice for FboInfoDevice {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn len(&self) -> usize {
        self.get().len()
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        self.get().as_bytes().read(offset, buf)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
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
