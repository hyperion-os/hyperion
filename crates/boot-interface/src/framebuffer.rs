pub struct FramebufferCreateInfo {
    pub buf: &'static mut [u8],
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
}
