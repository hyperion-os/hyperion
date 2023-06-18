#![no_std]

//

use hyperion_framebuffer::framebuffer::Framebuffer;
use hyperion_vfs::{FileDevice, IoResult};

//

pub struct FboDevice;

//

impl FileDevice for FboDevice {
    fn len(&self) -> usize {
        if let Some(fbo) = Framebuffer::get() {
            fbo.lock().buf_mut().len()
        } else {
            0
        }
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        let fbo = Framebuffer::get();
        let mut lock;
        let this = if let Some(fbo) = fbo {
            lock = fbo.lock();
            &*lock.buf_mut()
        } else {
            &[]
        };

        hyperion_vfs_util::slice_read(this, offset, buf)
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize> {
        let fbo = Framebuffer::get();
        let mut lock;
        let this = if let Some(fbo) = fbo {
            lock = fbo.lock();
            lock.buf_mut()
        } else {
            &mut []
        };

        hyperion_vfs_util::slice_write(this, offset, buf)
    }
}
