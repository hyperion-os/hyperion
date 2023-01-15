use super::font::FONT;
use core::{
    fmt,
    ops::{Deref, DerefMut},
};
use spin::{Lazy, Mutex, MutexGuard, Once};

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
            let two_rows = &mut self.buf[(y - 1) * self.info.pitch..(y + 1) * self.info.pitch];

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

impl Color {
    pub const WHITE: Color = Color::new(0xff, 0xff, 0xff);
    pub const BLACK: Color = Color::new(0x00, 0x00, 0x00);

    pub const RED: Color = Color::new(0xff, 0x00, 0x00);
    pub const GREEN: Color = Color::new(0x00, 0xff, 0x00);
    pub const BLUE: Color = Color::new(0x00, 0x00, 0xff);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn from_u32(code: u32) -> Self {
        let [r, g, b, _] = code.to_ne_bytes();
        Self::new(r, g, b)
    }

    pub const fn from_hex(hex_code: &str) -> Self {
        Self::from_hex_bytes(hex_code.as_bytes())
    }

    pub const fn from_hex_bytes(hex_code: &[u8]) -> Self {
        match hex_code {
            [r0, r1, g0, g1, b0, b1, _, _]
            | [r0, r1, g0, g1, b0, b1]
            | [b'#', r0, r1, g0, g1, b0, b1, _, _]
            | [b'#', r0, r1, g0, g1, b0, b1] => {
                Self::from_hex_bytes_2([*r0, *r1, *g0, *g1, *b0, *b1])
            }
            _ => {
                panic!("Invalid color hex code")
            }
        }
    }

    pub const fn from_hex_bytes_2(hex_code: [u8; 6]) -> Self {
        const fn parse_hex_char(c: u8) -> u8 {
            match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 0xa,
                _ => c,
            }
        }

        const fn parse_byte(str_byte: [u8; 2]) -> u8 {
            parse_hex_char(str_byte[0]) | parse_hex_char(str_byte[1]) << 4
        }

        let r = parse_byte([hex_code[0], hex_code[1]]);
        let g = parse_byte([hex_code[2], hex_code[3]]);
        let b = parse_byte([hex_code[4], hex_code[5]]);

        Self::new(r, g, b)
    }

    pub const fn as_u32(&self) -> u32 {
        // self.b as u32 | (self.g as u32) << 8 | (self.r as u32) << 16
        u32::from_le_bytes([self.b, self.g, self.r, 0])
    }

    pub const fn as_arr(&self) -> [u8; 4] {
        self.as_u32().to_ne_bytes()
        // [self.r, self.g, self.b, 0]
    }
}
