#![no_std]

//

use hyperion_framebuffer::framebuffer::Framebuffer;
use hyperion_vfs::{device::FileDevice, error::IoResult};

//

pub struct FboDevice;

//

impl FileDevice for FboDevice {
    fn len(&self) -> usize {
        Self::with(|fbo| fbo.len())
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        Self::with(|fbo| fbo.read(offset, buf))
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize> {
        Self::with_mut(|mut fbo| fbo.write(offset, buf))
    }
}

impl FboDevice {
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
