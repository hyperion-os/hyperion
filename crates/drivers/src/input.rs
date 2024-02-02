use core::any::Any;

use hyperion_events::{keyboard, mouse};
use hyperion_futures::block_on;
use hyperion_vfs::{
    device::FileDevice,
    error::{IoError, IoResult},
};

//

pub struct KeyboardDevice;

impl FileDevice for KeyboardDevice {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        0
    }

    fn set_len(&mut self, _: usize) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    fn read(&self, _: usize, buf: &mut [u8]) -> IoResult<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let s = block_on(async move { keyboard::buffer::recv_raw().await });
        buf[0] = s;

        Ok(1)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

//

pub struct MouseDevice;

impl FileDevice for MouseDevice {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        0
    }

    fn set_len(&mut self, _: usize) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    fn read(&self, _: usize, buf: &mut [u8]) -> IoResult<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let s = block_on(async move { mouse::buffer::recv_raw().await });
        let limit = buf.len().min(3);
        buf[..limit].copy_from_slice(&s[..limit]);

        Ok(limit)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}
