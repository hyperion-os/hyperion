use super::{color::Color, font::FONT};
use crate::{boot, debug};
use alloc::{boxed::Box, vec};
use core::{
    fmt, mem,
    ops::{Deref, DerefMut, Range},
    sync::atomic::{AtomicBool, Ordering},
};
use spin::{Lazy, Mutex, MutexGuard};

//

pub struct Framebuffer {
    /// video memory
    vmem: Option<&'static mut [u8]>,
    /// video memory / backbuffer
    buf: &'static mut [u8],

    flush_first: usize,
    flush_last: usize,

    pub info: FramebufferInfo,
}

pub struct FramebufferRaiiFlush {
    lock: MutexGuard<'static, Framebuffer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FramebufferInfo {
    pub width: usize, // not the pixels to the next row
    pub height: usize,
    pub pitch: usize, // pixels to the next row
}

//

impl Framebuffer {
    pub fn new(vmem: &'static mut [u8], info: FramebufferInfo) -> Self {
        Self {
            // buf: Box::leak(vec![0; vmem.len()].into_boxed_slice()),
            buf: vmem,
            vmem: None,

            flush_first: 0,
            flush_last: 0,

            info,
        }
    }

    pub fn get() -> Option<FramebufferRaiiFlush> {
        Some(FramebufferRaiiFlush {
            lock: Self::get_manual_flush()?,
        })
    }

    pub fn get_manual_flush() -> Option<MutexGuard<'static, Framebuffer>> {
        _ = Self::init_backbuffer();

        FBO.as_ref().map(|mtx| mtx.lock())
    }

    pub fn flush(&mut self) {
        if let Some(vmem) = &mut self.vmem {
            let from = &self.buf[self.flush_first..self.flush_last];
            let to = &mut vmem[self.flush_first..self.flush_last];

            unsafe {
                // https://doc.rust-lang.org/stable/core/intrinsics/fn.volatile_copy_nonoverlapping_memory.html
                core::ptr::copy_nonoverlapping(from.as_ptr(), to.as_mut_ptr(), to.len());
            }
        }
    }

    pub fn pixel(&mut self, x: usize, y: usize, color: Color) {
        let spot = self.pixel_keep_area(x, y, color);
        self.flush_area(spot);
    }

    pub fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, color: Color) {
        for yd in y..y + h {
            let spot = x * 4 + yd * self.pitch;
            self.buf[spot..spot + 4 * w]
                .as_chunks_mut::<4>()
                .0
                .fill(color.as_arr());
        }

        self.flush_area(x * 4 + y * self.pitch..(x + w) * 4 + (y + h) * self.pitch);
    }

    pub fn ascii_char(&mut self, x: usize, y: usize, ch: u8, fg: Color, bg: Color) -> bool {
        let (map, double_wide) = FONT[ch as usize];

        let (w, h) = (if double_wide { 16 } else { 8 }, 8);

        for (yd, row) in map.into_iter().enumerate() {
            for xd in 0..w {
                let px_col = if (row & 1 << xd) != 0 { fg } else { bg };
                self.pixel_keep_area(x + xd, y + yd, px_col);
            }
        }

        self.flush_area(x * 4 + y * self.pitch..(x + w) * 4 + (y + h) * self.pitch);

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

    fn pixel_keep_area(&mut self, x: usize, y: usize, color: Color) -> Range<usize> {
        let spot = x * 4 + y * self.pitch;
        self.buf[spot..spot + 4].copy_from_slice(&color.as_arr()[..]);
        spot..spot + 4
    }

    fn flush_area(&mut self, area: Range<usize>) {
        self.flush_first = self.flush_first.min(area.start);
        self.flush_last = self.flush_last.max(area.end);
    }

    fn init_backbuffer() -> Option<()> {
        static FBO_BUFFERED: AtomicBool = AtomicBool::new(true);

        if FBO_BUFFERED.swap(false, Ordering::SeqCst) {
            // get the allocation size and unlock the fbo
            let len = FBO.as_ref()?.lock().buf.len();
            // this alloc could trigger a deadlock in the
            // framebuffer logger if the fbo isnt unlocked before
            let mut buf = vec![0; len].into_boxed_slice();

            let mut this = FBO.as_ref()?.lock();
            buf.copy_from_slice(this.buf);
            this.vmem = Some(mem::take(&mut this.buf));
            this.buf = Box::leak(buf);
            drop(this);

            debug!("FBO backbuffer initialized");
        }

        Some(())
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Framebuffer")
            .field("info", &self.info)
            .finish()
    }
}

impl Deref for FramebufferRaiiFlush {
    type Target = Framebuffer;

    fn deref(&self) -> &Self::Target {
        &self.lock
    }
}

impl DerefMut for FramebufferRaiiFlush {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.lock
    }
}

impl Drop for FramebufferRaiiFlush {
    fn drop(&mut self) {
        self.flush();
    }
}

//

static FBO: Lazy<Option<Mutex<Framebuffer>>> = Lazy::new(|| {
    let mut fbo = boot::framebuffer()?;
    fbo.clear();
    fbo.flush();
    Some(Mutex::new(fbo))
});

//

#[cfg(test)]
mod tests {
    use super::Framebuffer;
    use crate::driver::video::color::Color;

    //

    #[test_case]
    fn fbo_draw() {
        if let Some(mut fbo) = Framebuffer::get() {
            fbo.fill(440, 340, 40, 40, Color::RED);
            fbo.fill(450, 350, 60, 40, Color::GREEN);
            fbo.fill(405, 315, 80, 20, Color::BLUE);
        }
    }
}
