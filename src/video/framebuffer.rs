use super::{color::Color, font::FONT};
use core::ops::{Deref, DerefMut};
use spin::{Mutex, MutexGuard, Once};

//

pub static FBO: Once<Mutex<Framebuffer>> = Once::new();

//

pub fn get_fbo() -> Option<MutexGuard<'static, Framebuffer>> {
    FBO.get().map(|mtx| mtx.lock())
}

//

pub struct Framebuffer {
    pub buf: &'static mut [u8],

    pub info: FramebufferInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FramebufferInfo {
    pub width: usize, // not the pixels to the next row
    pub height: usize,
    pub pitch: usize, // pixels to the next row
}

//

impl Framebuffer {
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

//

#[cfg(test)]
mod tests {
    use super::get_fbo;
    use crate::video::color::Color;

    //

    #[test_case]
    fn fbo_draw() {
        if let Some(mut fbo) = get_fbo() {
            fbo.fill(440, 340, 40, 40, Color::RED);
            fbo.fill(450, 350, 60, 40, Color::GREEN);
            fbo.fill(405, 315, 80, 20, Color::BLUE);
        }
    }
}
