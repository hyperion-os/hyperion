use core::ptr;

//

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct VirtAddr(pub x86_64::VirtAddr);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PhysAddr(pub x86_64::PhysAddr);

//

impl VirtAddr {
    /// # Safety
    ///
    /// * `self` must be [valid] for writes.
    ///
    /// * `self` must be properly aligned.
    pub unsafe fn write_volatile(self, data: impl Sized) {
        ptr::write_volatile(self.0.as_mut_ptr(), data);
    }

    /// # Safety
    ///
    /// * `self` must be [valid] for reads.
    ///
    /// * `self` must be properly aligned.
    ///
    /// * `self` must point to a properly initialized value of type `T`.
    pub unsafe fn read_volatile<T: Copy>(self) -> T {
        ptr::read_volatile(self.0.as_ptr())
    }
}
