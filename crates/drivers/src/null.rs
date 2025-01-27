use core::any::Any;

use hyperion_vfs::{device::FileDevice, Result};

//

pub struct Null;

impl FileDevice for Null {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        1
    }

    fn read(&self, _: usize, _: &mut [u8]) -> Result<usize> {
        Ok(0)
    }

    fn write(&mut self, _: usize, buf: &[u8]) -> Result<usize> {
        Ok(buf.len())
    }
}
