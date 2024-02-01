use alloc::boxed::Box;
use core::fmt;

use hyperion_color::Color;
use hyperion_framebuffer::framebuffer::Framebuffer;
use hyperion_log::LogLevel;

use super::CHAR_SIZE;

//

pub struct Term {
    pub cursor: (usize, usize),
    pub size: (usize, usize),
    buf: Box<[u8]>,
    old_buf: Box<[u8]>,
}

//

impl Term {
    pub fn new() -> Self {
        hyperion_log_multi::set_fbo(LogLevel::None);
        let Some(vbo) = Framebuffer::get() else {
            // TODO: serial only
            panic!("cannot run kshell without a framebuffer");
        };
        let mut vbo = vbo.lock();
        vbo.clear();

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
        }
    }

    pub fn flush(&mut self) {
        // framebuffer is already proven to be Some
        let mut vbo = Framebuffer::get().unwrap().lock();

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
            vbo.ascii_char(x, y, *ch, Color::WHITE, Color::BLACK);
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
        self.flush();

        // let mut fbo = Framebuffer::get().unwrap().lock();
        // let (w, h) = (fbo.width, fbo.height);
        // fbo.fill(0, 0, w, h, Color::BLACK);
    }

    pub fn read_at(&self, cursor: (usize, usize)) -> u8 {
        self.buf[cursor.0 + cursor.1 * self.size.0]
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

impl Default for Term {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Write for Term {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());
        Ok(())
    }
}
