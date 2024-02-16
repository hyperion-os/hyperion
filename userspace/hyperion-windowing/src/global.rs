use std::{
    fs::{File, OpenOptions},
    intrinsics::volatile_copy_nonoverlapping_memory,
    io::{BufRead, BufReader},
    marker::PhantomData,
    ptr::{self, NonNull},
};

use hyperion_syscall::{fs::FileDesc, map_file, unmap_file};

use crate::os::AsRawFd;

//

#[derive(Debug, Clone, Copy)]
pub struct Region<'a> {
    pub buf: *mut u32,
    pub pitch: usize,  // offset to the next line aka. the real width
    pub width: usize,  // width of the region
    pub height: usize, // height of the region

    _p: PhantomData<&'a *mut u32>,
}

unsafe impl Sync for Region<'_> {}
unsafe impl Send for Region<'_> {}

impl<'a> Region<'a> {
    /// # Safety
    /// buf must be aligned and point to a valid allocation of at least `pitch * height * 4` bytes
    pub const unsafe fn new(buf: *mut u32, pitch: usize, width: usize, height: usize) -> Self {
        Self {
            buf,
            pitch,
            width,
            height,
            _p: PhantomData,
        }
    }

    // fn sub_region(self, x: usize, y: usize, width: usize, height: usize) -> Option<Region> {
    //     if x >= self.pitch {
    //         panic!();
    //     }
    //     if y >= self.height {
    //         panic!();
    //     }
    // }

    pub fn volatile_copy_from(&mut self, from: &Region, to_x: isize, to_y: isize) {
        // https://gdbooks.gitbooks.io/3dcollisions/content/Chapter2/static_aabb_aabb.html

        // let xmin_a = 0usize; // left
        // let ymin_a = 0usize; // up
        let xmax_a = self.width; // right
        let ymax_a = self.height; // down

        let xmin_b = to_x.max(0) as usize; // left
        let ymin_b = to_y.max(0) as usize; // up
        let xmax_b = (to_x + from.width as isize).max(0) as usize; // right
        let ymax_b = (to_y + from.height as isize).max(0) as usize; // down

        let xmin = xmin_b;
        let ymin = ymin_b;
        let xmax = xmax_a.min(xmax_b);
        let ymax = ymax_a.min(ymax_b);

        let x = xmin;
        let x_len = xmax - x;
        let y = ymin;
        let y_len = ymax - y;

        if x_len <= 0 || y_len <= 0 {
            return;
        }

        assert!(xmax <= self.width);
        assert!(ymax <= self.height);

        assert!(x as isize - to_x >= 0);
        assert!(y as isize - to_y >= 0);
        assert!(xmax.checked_add_signed(-to_x).unwrap() <= from.width);
        assert!(ymax.checked_add_signed(-to_y).unwrap() <= from.height);

        for y in ymin..ymax {
            let to_spot = x + y * self.pitch;
            let from_spot =
                x.wrapping_add_signed(-to_x) + y.wrapping_add_signed(-to_y) * from.pitch;

            let to = unsafe { self.buf.add(to_spot) };
            let from = unsafe { from.buf.add(from_spot) };

            unsafe {
                volatile_copy_nonoverlapping_memory(to, from, x_len);
            }
        }
    }

    pub fn volatile_fill(&mut self, x: usize, y: usize, w: usize, h: usize, col: u32) {
        let x_len = self.width.min(x + w).saturating_sub(x);
        let y_len = self.height.min(y + h).saturating_sub(y);

        if x_len <= 0 || y_len <= 0 {
            return;
        }

        for y in y..y + y_len {
            for x in x..x + x_len {
                let to_spot = x + y * self.pitch;
                let to = unsafe { self.buf.add(to_spot) };

                unsafe { to.write_volatile(col) }
            }
        }
    }
}

//

#[allow(unused)]
pub struct GlobalFb {
    pub width: usize,
    pub height: usize,
    pub pitch: usize,

    pub buf: *mut [u8],
    pub fbo: File,
    pub fbo_fd: FileDesc,
    pub fbo_mapped: NonNull<()>,
}

impl GlobalFb {
    pub fn lock_global_fb() -> GlobalFb {
        let info = framebuffer_info();

        let fbo = OpenOptions::new()
            .write(true)
            .open("/dev/fb0")
            .expect("failed to open /dev/fb0");
        let meta = fbo.metadata().expect("failed to read fb file metadata");

        let fbo_fd = FileDesc(AsRawFd::as_raw_fd(&fbo) as _);

        let fbo_mapped: NonNull<()> =
            map_file(fbo_fd, None, meta.len() as _, 0).expect("failed to map the fb");

        let buf = ptr::slice_from_raw_parts_mut(fbo_mapped.as_ptr().cast(), meta.len() as _);

        GlobalFb {
            width: info.width,
            height: info.height,
            pitch: info.pitch,

            buf,
            fbo,
            fbo_fd,
            fbo_mapped,
        }
    }

    pub const fn buf_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *self.buf }
    }

    pub const fn as_region(&mut self) -> Region<'_> {
        let buf = self.fbo_mapped.as_ptr().cast();
        // SAFETY: GlobalFb is borrowed for the lifetime of Region<'_>
        // because GlobalFb owns the buffer mapping and automatically frees it
        unsafe { Region::new(buf, self.pitch / 4, self.width, self.height) }
    }
}

impl Drop for GlobalFb {
    fn drop(&mut self) {
        unmap_file(self.fbo_fd, self.fbo_mapped, 0).expect("failed to unmap the fb");
    }
}

//

fn framebuffer_info() -> Framebuffer {
    let fbo_info = OpenOptions::new().read(true).open("/dev/fb0-info").unwrap();
    let fbo_info = BufReader::new(fbo_info);

    let line = fbo_info.lines().next().unwrap().unwrap();

    let mut fbo_info_iter = line.split(':');
    let width = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();
    let height = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();
    let pitch = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();
    // let bpp = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();

    Framebuffer {
        width,
        height,
        pitch,
    }
}

#[derive(Debug)]
struct Framebuffer {
    width: usize,
    height: usize,
    pitch: usize,
}
