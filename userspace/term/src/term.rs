use std::fmt;

use hyperion_color::Color;
use hyperion_escape::decode::{DecodedPart, EscapeDecoder};
use hyperion_windowing::global::Region;

use crate::font::{MonoFont, MonoGlyph};

//

pub struct Term<'a> {
    escapes: EscapeDecoder,

    pub stdout_cursor: (usize, usize),
    pub cursor: (usize, usize),
    pub size: (usize, usize),
    buf: Box<[u8]>,
    old_buf: Box<[u8]>,

    fbo: Region<'a>,
    font: MonoFont,
}

//

impl<'a> Term<'a> {
    pub fn new(fbo: Region<'a>, font: MonoFont) -> Self {
        let cursor = (0, 0);

        let size = (fbo.width / 8, fbo.height / 16);

        let buf = (0..size.0 * size.1).map(|_| b' ').collect();
        let old_buf = (0..size.0 * size.1).map(|_| b'=').collect();

        Self {
            escapes: EscapeDecoder::new(),

            stdout_cursor: cursor,
            cursor,
            size,
            buf,
            old_buf,

            fbo,
            font,
        }
    }

    pub fn flush(&mut self) {
        // let mut updates = 0u32;
        for ((idx, ch), _) in self
            .buf
            .iter()
            .enumerate()
            .zip(self.old_buf.iter())
            .filter(|((_, b1), b0)| **b1 != **b0)
        {
            let x = (idx % self.size.0) * 8;
            let y = (idx / self.size.0) * 16;

            let mut fg = Color::WHITE;
            let bg = Color::BLACK;

            let mut glyph = self.font.glyph(*ch);
            if glyph.is_wide {
                glyph = self.font.glyph(b'?');
                fg = Color::RED;
            }

            // updates += 1;
            Self::ascii_char(&mut self.fbo, x, y, glyph, fg.as_u32(), bg.as_u32());
        }
        // debug!("updates: {updates}");
        self.old_buf.copy_from_slice(&self.buf);
    }

    pub fn ascii_char(fbo: &mut Region, x: usize, y: usize, glyph: MonoGlyph, fg: u32, bg: u32) {
        // each glyph is 512 bytes, should all 256 of them be cached or created on the go
        let mut glyph_region = [0u32; 8 * 16];

        for (y, row) in glyph.bitmap.iter().enumerate() {
            for x in 0u16..8 {
                glyph_region[x as usize + y * 8] = if *row & 1 << x != 0 { fg } else { bg };
            }
        }

        let glyph_region = unsafe { Region::new(glyph_region.as_mut_ptr(), 8, 8, 16) };

        fbo.volatile_copy_from(&glyph_region, x as _, y as _);
    }

    // let spot = x * 4 + y * self.pitch;
    // self.buf[spot..spot + 4].copy_from_slice(&color.as_arr()[..]);
    // spot..spot + 4

    /* pub fn cursor_next(&mut self) {
        self.cursor.0 += 1;

        if self.cursor.0 >= self.size.0 {
            self.cursor.0 = 0;
            self.cursor.1 += 1;
        }
    } */

    pub fn write_bytes(&mut self, b: &[u8]) {
        for b in b {
            self.write_byte(*b);
        }
    }

    pub fn write_byte(&mut self, b: u8) {
        match self.escapes.next(b) {
            DecodedPart::Byte(b) => self.write_raw_byte(b),
            DecodedPart::Bytes(b) => {
                for b in b.into_iter().take_while(|b| *b != 0) {
                    self.write_raw_byte(b)
                }
            }
            DecodedPart::FgColor(_) => {}
            DecodedPart::BgColor(_) => {}
            DecodedPart::Reset => {}
            DecodedPart::CursorUp(n) => self.cursor.1 = self.cursor.1.saturating_sub(n as usize),
            DecodedPart::CursorDown(n) => {
                self.cursor.1 += n as usize;
                self.cursor.1 = self.cursor.1.min(self.size.1);
            }
            DecodedPart::CursorLeft(n) => self.cursor.0 = self.cursor.0.saturating_sub(n as usize),
            DecodedPart::CursorRight(n) => {
                self.cursor.0 += n as usize;
                self.cursor.0 = self.cursor.0.min(self.size.0);
            }
            DecodedPart::None => {}
        }
    }

    pub fn write_raw_byte(&mut self, b: u8) {
        if self.cursor.0 >= self.size.0 {
            self.cursor.0 = 0;
            self.cursor.1 += 1;
        }
        if self.cursor.1 >= self.size.1 {
            let len = self.buf.len();
            self.stdout_cursor.1 = self.stdout_cursor.1.saturating_sub(1);
            self.cursor.1 -= 1;
            self.buf.copy_within(self.size.0.., 0);
            self.buf[len - self.size.0..].fill(b' ');
        }

        // crate::debug!("{b}");
        match b {
            b'\n' => {
                self.cursor.0 = 0;
                self.cursor.1 += 1;
            }
            other => {
                self.buf[self.cursor.0 + self.cursor.1 * self.size.0] = other;
                self.cursor.0 += 1;
            }
        }
    }
}

impl fmt::Write for Term<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());
        Ok(())
    }
}
