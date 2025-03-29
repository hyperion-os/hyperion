use alloc::boxed::Box;
use core::mem::MaybeUninit;

use async_trait::async_trait;
use hyperion_arch::vmm::PageMap;
use hyperion_mem::buf::{Buffer, BufferMut};
use hyperion_random::Rng;
use hyperion_scheduler::proc::Process;
use hyperion_syscall::err::Result;
use hyperion_vfs::node::{FileDriver, Ref};

//

pub static DEV_RANDOM: Ref<dyn FileDriver> = Ref::new_static(&Random);

//

/// `/dev/random` which just reads from the rng device
pub struct Random;

#[async_trait]
impl FileDriver for Random {
    async fn read(
        &self,
        _: Option<&Process>,
        _: usize,
        mut buf: BufferMut<'_, u8, PageMap>,
    ) -> Result<usize> {
        unsafe {
            buf.with_slice_mut(|s| {
                let s = MaybeUninit::fill(s, 0); // fill with 0's first, because Rust
                hyperion_random::next_fast_rng().fill(s);
            });
        }

        Ok(buf.len())
    }

    async fn write(
        &self,
        _: Option<&Process>,
        _: usize,
        buf: Buffer<'_, u8, PageMap>,
    ) -> Result<usize> {
        Ok(buf.len())
    }
}
