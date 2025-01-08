use core::any::Any;

use hyperion_random::RngCore;
use hyperion_vfs::node::FileDriver;

//

pub struct Random;

// #[async_trait]
impl FileDriver for Random {
    // fn as_any(&self) -> &dyn Any {
    //     self
    // }

    // fn len(&self) -> usize {
    //     1
    // }

    // fn set_len(&mut self, _: usize) -> IoResult<()> {
    //     Err(IoError::PermissionDenied)
    // }

    //     async fn read(&self, _: usize, buf: &mut [u8]) -> Result<usize> {
    //         let mut rng = hyperion_random::next_fast_rng();
    //         rng.fill_bytes(buf);
    //         Ok(buf.len())
    //     }

    //     async fn write(&mut self, _: usize, buf: &[u8]) -> Result<usize> {
    //         Ok(buf.len())
    //     }
}
