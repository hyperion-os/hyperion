use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader},
    ptr::{self, NonNull},
};

use hyperion_syscall::{fs::FileDesc, map_file, unmap_file};

use crate::os::AsRawFd;

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

    pub fn buf_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *self.buf }
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
