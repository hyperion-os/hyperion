use core::any::Any;

use hyperion_random::RngCore;
use hyperion_vfs::{device::FileDevice, Result};

//

pub struct Random;

impl FileDevice for Random {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        1
    }

    fn read(&self, _: usize, buf: &mut [u8]) -> Result<usize> {
        let mut rng = hyperion_random::next_fast_rng();
        rng.fill_bytes(buf);
        Ok(buf.len())
    }
}
