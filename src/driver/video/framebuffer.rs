use super::{color::Color, font::FONT};
use crate::boot;
use alloc::{boxed::Box, vec};
use core::{
    fmt,
    ops::{Deref, DerefMut},
};
use spin::{Lazy, Mutex, MutexGuard};

//

pub fn get() -> Option<MutexGuard<'static, Framebuffer>> {
    FBO.as_ref().map(|mtx| mtx.lock())
}

//

pub struct Framebuffer {
    /// video memory
    vmem: &'static mut [u8],
    /// backbuffer
    buf: Box<[u8]>,

    pub info: FramebufferInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FramebufferInfo {
    pub width: usize, // not the pixels to the next row
    pub height: usize,
    pub pitch: usize, // pixels to the next row
}

//

static FBO: Lazy<Option<Mutex<Framebuffer>>> = Lazy::new(|| {
    let fbo = boot::framebuffer();
    let mut fbo = fbo?;
    fbo.clear();
    Some(Mutex::new(fbo))
});

//

impl Framebuffer {
    pub fn new(vmem: &'static mut [u8], info: FramebufferInfo) -> Self {
        Self {
            vmem,
            buf: vec![0; vmem.len()].into_boxed_slice(),
            info,
        }
    }

    pub fn flush(&mut self) {
        // https://doc.rust-lang.org/stable/core/intrinsics/fn.volatile_copy_nonoverlapping_memory.html
        assert_eq!(self.buf.len(), self.vmem.len());
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.buf.as_ptr(),
                self.vmem.as_mut_ptr(),
                self.vmem.len(),
            );
        }
    }

    pub fn set(&mut self, x: usize, y: usize, color: Color) {
        let spot = x * 4 + y * self.pitch;
        self.buf[spot..spot + 4].copy_from_slice(&color.as_arr()[..]);
    }

    pub fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, color: Color) {
        for yd in 0..h {
            for xd in 0..w {
                self.set(x + xd, y + yd, color);
            }
        }
    }

    pub fn put_byte(&mut self, x: usize, y: usize, ch: u8, fg: Color, bg: Color) -> bool {
        let (map, double_wide) = FONT[ch as usize];

        for (yd, row) in map.into_iter().enumerate() {
            for xd in 0..if double_wide { 16 } else { 8 } {
                self.set(x + xd, y + yd, if (row & 1 << xd) != 0 { fg } else { bg });
            }
        }

        double_wide
    }

    pub fn scroll(&mut self, h: usize) {
        for y in h..self.height {
            let _two_rows = &mut self.buf[(y - 1) * self.info.pitch..(y + 1) * self.info.pitch];

            self.buf.copy_within(
                y * self.info.pitch..(y + 1) * self.info.pitch,
                (y - h) * self.info.pitch,
            );
        }

        self.buf[(self.info.height - h) * self.info.pitch..].fill(0);
    }

    pub fn clear(&mut self) {
        self.buf.fill(0);
    }

    pub fn info(&self) -> FramebufferInfo {
        self.info
    }
}

impl Deref for Framebuffer {
    type Target = FramebufferInfo;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl DerefMut for Framebuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.info
    }
}

impl fmt::Debug for Framebuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Framebuffer")
            .field("info", &self.info)
            .finish()
    }
}

//

#[cfg(test)]
mod tests {
    use super::get;
    use crate::video::color::Color;

    //

    #[test_case]
    fn fbo_draw() {
        if let Some(mut fbo) = get() {
            fbo.fill(440, 340, 40, 40, Color::RED);
            fbo.fill(450, 350, 60, 40, Color::GREEN);
            fbo.fill(405, 315, 80, 20, Color::BLUE);
        }
    }
}
