use hyperion_driver_acpi::hpet::HPET;
use hyperion_vfs::{device::FileDevice, Result};

//

pub struct HpetDevice;

//

impl FileDevice for HpetDevice {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn len(&self) -> usize {
        core::mem::size_of::<i64>()
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        HPET.now_bytes()[..].read(offset, buf)
    }
}
