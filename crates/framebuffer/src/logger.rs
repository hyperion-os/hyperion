use core::fmt::{self, Arguments, Write};

use hyperion_color::Color;
use hyperion_escape::decode::{DecodedPart, EscapeDecoder};
use spin::{Mutex, MutexGuard};

use super::{font::FONT, framebuffer::Framebuffer};

//

pub fn _print(args: Arguments) {
    // TODO: without ints
    // without_interrupts(|| {
    if let Some(fbo) = Framebuffer::get() {
        let fbo = fbo.lock();
        _ = WriterLock {
            lock: WRITER.lock(),
            fbo,
        }
        .write_fmt(args)
    }
    // });
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

struct WriterLock {
    lock: MutexGuard<'static, Writer>,
    fbo: MutexGuard<'static, Framebuffer>,
}

//

impl Writer {
    fn write_bytes(&mut self, bytes: &[u8], fbo: &mut Framebuffer) {
        for byte in bytes {
            self.write_byte(*byte, fbo);
        }
    }

    fn write_byte(&mut self, byte: u8, fbo: &mut Framebuffer) {
        match self.escapes.next(byte) {
            DecodedPart::Byte(b'\n') => {
                #[cfg(debug_assertions)]
                let lines = if self.cursor[1] + 1 >= Self::size(fbo)[1] {
                    // scroll more if the cursor is near the bottom
                    //
                    // because scrolling is slow in debug mode
                    8
                } else {
                    1
                };
                #[cfg(not(debug_assertions))]
                let lines = 1;
                self.new_line(lines, fbo)
            }
            DecodedPart::Byte(b'\t') => {
                self.cursor[0] = (self.cursor[0] / 4 + 1) * 4;
            }

            DecodedPart::Byte(byte) => self.write_byte_raw(byte, fbo),
            DecodedPart::Bytes(bytes) => bytes
                .into_iter()
                .take_while(|b| *b != 0)
                .for_each(|byte| self.write_byte_raw(byte, fbo)),

            DecodedPart::FgColor(color) => self.fg_color = color,
            DecodedPart::BgColor(color) => self.bg_color = color,
            DecodedPart::Reset => {
                self.fg_color = Self::FG_COLOR;
                self.bg_color = Self::BG_COLOR;
            }

            DecodedPart::None => {}
        }
    }

    fn write_byte_raw(&mut self, byte: u8, fbo: &mut Framebuffer) {
        let size = Self::size(fbo);
        if size[0] == 0 || size[1] == 0 {
            return;
        }

        let is_double = FONT[byte as usize].1;

        // insert a new line if the next character would be off screen
        if self.cursor[0] + if is_double { 1 } else { 0 } >= Self::size(fbo)[0] {
            self.new_line(8, fbo);
        }

        let (x, y) = (self.cursor[0] as usize * 8, self.cursor[1] as usize * 16);
        self.cursor[0] += if is_double { 2 } else { 1 };

        fbo.ascii_char(x, y, byte, self.fg_color, self.bg_color);
    }

    const FG_COLOR: Color = Color::from_hex("#bbbbbb").unwrap();
    const BG_COLOR: Color = Color::from_hex("#000000").unwrap();

    const fn new() -> Self {
        Self {
            cursor: [0; 2],
            fg_color: Self::FG_COLOR,
            bg_color: Self::BG_COLOR,

            escapes: EscapeDecoder::new(),
        }
    }

    fn new_line(&mut self, count: u16, fbo: &mut Framebuffer) {
        self.cursor[0] = 0;
        self.cursor[1] += 1;
        if self.cursor[1] >= Self::size(fbo)[1] {
            let scroll_count = count.min(self.cursor[1]);
            self.cursor[1] -= scroll_count;
            fbo.scroll(16 * scroll_count as usize);
        }
    }

    fn size(fbo: &Framebuffer) -> [u16; 2] {
        [(fbo.width / 8) as _, (fbo.height / 16) as _]
    }
}

impl fmt::Write for WriterLock {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.lock.write_bytes(s.as_bytes(), &mut self.fbo);
        Ok(())
    }
}
