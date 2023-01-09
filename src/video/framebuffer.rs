use super::font::FONT;
use core::fmt;
use spin::{Mutex, Once};

//

pub static FBO: Once<Mutex<Framebuffer>> = Once::new();

//

pub struct Framebuffer {
    pub buf: &'static mut [u8],

    pub width: usize, // not the pixels to the next row
    pub height: usize,
    pub pitch: usize, // pixels to the next row
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
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

    pub fn put_byte(&mut self, x: usize, y: usize, ch: u8, fg: Color, bg: Color) {
        let map = FONT[ch as usize];

        for (yd, row) in map.into_iter().enumerate() {
            for xd in 0..8 {
                self.set(
                    x + xd,
                    y + yd,
                    if (row & 1 << (7 - xd)) != 0 { fg } else { bg },
                );
            }
        }
    }

    pub fn put_bytes(&mut self, x: usize, y: usize, s: &[u8], fg: Color, bg: Color) {
        for (offs, ch) in s.iter().enumerate() {
            self.put_byte(x + 12 * offs, y, *ch, fg, bg)
        }
    }
}

impl Color {
    pub const WHITE: Color = Color::new(0xff, 0xff, 0xff);
    pub const BLACK: Color = Color::new(0x00, 0x00, 0x00);

    pub const RED: Color = Color::new(0xff, 0x00, 0x00);
    pub const GREEN: Color = Color::new(0x00, 0xff, 0x00);
    pub const BLUE: Color = Color::new(0x00, 0x00, 0xff);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn as_u32(&self) -> u32 {
        u32::from_ne_bytes([self.r, self.g, self.b, 0])
    }

    pub const fn as_arr(&self) -> [u8; 4] {
        [self.r, self.g, self.b, 0]
    }
}

impl fmt::Debug for Framebuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Framebuffer")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pitch", &self.pitch)
            .finish_non_exhaustive()
    }
}
