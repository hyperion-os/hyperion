use super::CHAR_SIZE;
use crate::driver::video::{color::Color, framebuffer::Framebuffer};
use alloc::boxed::Box;
use core::fmt;

//

pub struct Term<'fbo> {
    pub cursor: (usize, usize),
    size: (usize, usize),
    buf: Box<[u8]>,
    old_buf: Box<[u8]>,
    vbo: &'fbo mut Framebuffer,
}

//

impl<'fbo> Term<'fbo> {
    pub fn new(vbo: &'fbo mut Framebuffer) -> Self {
        let vbo_info = vbo.info();

        let cursor = (0, 0);

        let size = (
            vbo_info.width / CHAR_SIZE.0 as usize,
            vbo_info.height / CHAR_SIZE.1 as usize,
        );

        let buf = (0..size.0 * size.1).map(|_| b' ').collect();
        let old_buf = (0..size.0 * size.1).map(|_| b'=').collect();

        Self {
            cursor,
            size,
            buf,
            old_buf,
            vbo,
        }
    }

    pub fn flush(&mut self) {
        // let positions = (0..self.size.1).flat_map(|y| (0..self.size.0).map(move |x| (x, y)));

        // let mut updates = 0u32;
        for ((idx, ch), _) in self
            .buf
            .iter()
            .enumerate()
            .zip(self.old_buf.iter())
            .filter(|((_, b1), b0)| **b1 != **b0)
        {
            let x = (idx % self.size.0) * CHAR_SIZE.0 as usize;
            let y = (idx / self.size.0) * CHAR_SIZE.1 as usize;

            // updates += 1;
            self.vbo.ascii_char(x, y, *ch, Color::WHITE, Color::BLACK);
        }
        // debug!("updates: {updates}");
        self.old_buf.copy_from_slice(&self.buf);
    }

    /* pub fn cursor_next(&mut self) {
        self.cursor.0 += 1;

        if self.cursor.0 >= self.size.0 {
            self.cursor.0 = 0;
            self.cursor.1 += 1;
        }
    } */

    pub fn cursor_prev(&mut self) {
        if self.cursor.0 == 0 {
            if self.cursor.1 == 0 {
                return;
            }
            self.cursor.0 = self.size.0 - 1;
            self.cursor.1 -= 1;
        }

        self.cursor.0 -= 1;
    }

    pub fn write_bytes(&mut self, b: &[u8]) {
        for b in b {
            self.write_byte(*b);
        }
    }

    pub fn clear(&mut self) {
        self.cursor = (0, 0);
        self.buf.fill(b' ');
    }

    pub fn write_byte(&mut self, b: u8) {
        if self.cursor.0 >= self.size.0 {
            self.cursor.0 = 0;
            self.cursor.1 += 1;
        }
        if self.cursor.1 >= self.size.1 {
            let len = self.buf.len();
            self.cursor.1 = self.size.1 - 1;
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

impl<'fbo> fmt::Write for Term<'fbo> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());
        Ok(())
    }
}
