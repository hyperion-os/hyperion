use core::any::Any;

use hyperion_random::RngCore;
use hyperion_vfs::{
    device::FileDevice,
    error::{IoError, IoResult},
};

//

pub struct Random;

impl FileDevice for Random {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        1
    }

    fn set_len(&mut self, _: usize) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    fn read(&self, _: usize, buf: &mut [u8]) -> IoResult<usize> {
        let mut rng = hyperion_random::next_fast_rng();
        rng.fill_bytes(buf);
        Ok(buf.len())
    }

    fn write(&mut self, _: usize, buf: &[u8]) -> IoResult<usize> {
        Ok(buf.len())
    }
}
