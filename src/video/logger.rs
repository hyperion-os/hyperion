use crate::term::escape::decode::{DecodedPart, EscapeDecoder};

use super::{
    font::FONT,
    framebuffer::{get_fbo, Color, Framebuffer},
};
use core::fmt::{self, Arguments, Write};
use spin::{Mutex, MutexGuard};

//

pub fn _print(args: Arguments) {
    _ = WRITER.lock().write_fmt(args)
}

//

static WRITER: Mutex<Writer> = Mutex::new(Writer::new());

//

struct Writer {
    cursor: [u16; 2],
    fg_color: Color,
    bg_color: Color,

    escapes: EscapeDecoder,
}

//

impl Writer {
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.write_byte(*byte)
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match self.escapes.next(byte) {
            DecodedPart::Byte(b'\n') => {
                if let Some(mut fbo) = get_fbo() {
                    self.new_line(1, &mut fbo)
                }
            }
            DecodedPart::Byte(b'\t') => {
                self.cursor[0] = (self.cursor[0] / 4 + 1) * 4;
            }

            DecodedPart::Byte(byte) => self.write_byte_raw(byte),
            DecodedPart::Bytes(bytes) => bytes
                .into_iter()
                .take_while(|b| *b != 0)
                .for_each(|byte| self.write_byte_raw(byte)),

            DecodedPart::FgColor(color) => self.fg_color = color,
            DecodedPart::BgColor(color) => self.bg_color = color,
            DecodedPart::Reset => {
                self.fg_color = Self::FG_COLOR;
                self.bg_color = Self::BG_COLOR;
            }

            DecodedPart::None => {}
        }
    }

    pub fn write_byte_raw(&mut self, byte: u8) {
        if let Some(mut fbo) = get_fbo() {
            let size = Self::size(&mut fbo);
            if size[0] == 0 || size[1] == 0 {
                return;
            }

            self._write_byte_raw(byte, &mut fbo);
        }
    }

    const FG_COLOR: Color = Color::from_hex("#bbbbbb");
    const BG_COLOR: Color = Color::from_hex("#000000");

    const fn new() -> Self {
        Self {
            cursor: [0; 2],
            fg_color: Self::FG_COLOR,
            bg_color: Self::BG_COLOR,

            escapes: EscapeDecoder::new(),
        }
    }

    fn _write_byte_raw(&mut self, byte: u8, fbo: &mut MutexGuard<Framebuffer>) {
        let (map, is_double) = FONT[byte as usize];

        // insert a new line if the next character would be off screen
        if self.cursor[0] + if is_double { 1 } else { 0 } >= Self::size(fbo)[0] {
            self.new_line(8, fbo);
        }

        let (x, y) = (self.cursor[0] as usize * 8, self.cursor[1] as usize * 16);
        self.cursor[0] += if is_double { 2 } else { 1 };

        for (yd, row) in map.into_iter().enumerate() {
            for xd in 0..if is_double { 16 } else { 8 } {
                fbo.set(
                    x + xd,
                    y + yd,
                    if (row & 1 << xd) != 0 {
                        self.fg_color
                    } else {
                        self.bg_color
                    },
                );
            }
        }
    }

    fn new_line(&mut self, count: u16, fbo: &mut MutexGuard<Framebuffer>) {
        self.cursor[0] = 0;
        self.cursor[1] += 1;
        if self.cursor[1] >= Self::size(fbo)[1] {
            let scroll_count = count.min(self.cursor[1]);
            self.cursor[1] -= scroll_count;
            fbo.scroll(16 * scroll_count as usize);
        }
    }

    fn size(fbo: &mut MutexGuard<Framebuffer>) -> [u16; 2] {
        [(fbo.width / 16) as _, (fbo.height / 16) as _]
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());
        Ok(())
    }
}
