#[derive(Debug, Clone, Copy)]
pub struct FramebufferCreateInfo {
    pub buf: *mut [u8],
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
}

unsafe impl Sync for FramebufferCreateInfo {}
unsafe impl Send for FramebufferCreateInfo {}

impl FramebufferCreateInfo {
    /// # Safety
    /// this is not synchronized between copies of [`FramebufferCreateInfo`]
    pub unsafe fn buf_mut(&mut self) -> &'static mut [u8] {
        unsafe { &mut *self.buf }
    }
}
