#![no_std]
#![no_main]
#![feature(slice_as_chunks)]

//

use core::slice;

use hyperion_color::Color;
use libstd::{
    alloc::{string::String, vec},
    fs::OpenOptions,
    io::BufReader,
    println,
    sys::*,
    CliArgs,
};

//

fn framebuffer_info() -> Framebuffer<'static> {
    let fbo_info = OpenOptions::new().read(true).open("/dev/fb0-info").unwrap();
    let mut fbo_info = BufReader::new(fbo_info);

    let mut buf = String::new();
    fbo_info.read_line(&mut buf).unwrap();
    drop(fbo_info);

    let mut fbo_info_iter = buf.split(':');
    let width = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();
    let height = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();
    let pitch = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();
    // let bpp = fbo_info_iter.next().unwrap().parse::<usize>().unwrap();

    Framebuffer {
        width,
        height,
        pitch,
        buf: &mut [],
    }
}

fn drawing(mut fb: Framebuffer, buf: &mut [u8]) {
    let (w, h) = (fb.width, fb.height);

    fb.fill(0, 0, w, h, Color::from_hex("#222222").unwrap());

    for i in 0..1000 {
        let i = i as f32 * 0.1;
        let x0 = (250.0 + libm::cosf(i) * 25.0) as usize;
        let y0 = (250.0 + libm::sinf(i) * 25.0) as usize;
        let x1 = (250.0 - libm::cosf(i) * 25.0) as usize;
        let y1 = (250.0 - libm::sinf(i) * 25.0) as usize;

        fb.fill(x0, y0, 20, 20, Color::from_hex("#00FFFF").unwrap());
        fb.fill(x1, y1, 20, 20, Color::from_hex("#00FFFF").unwrap());

        buf.copy_from_slice(fb.buf);
        nanosleep(5_000_000);

        fb.fill(x0, y0, 20, 20, Color::from_hex("#222222").unwrap());
        fb.fill(x1, y1, 20, 20, Color::from_hex("#222222").unwrap());
    }

    fb.fill(0, 0, w, h, Color::from_hex("#000000").unwrap());
    buf.copy_from_slice(fb.buf);
}

#[derive(Debug)]
struct Framebuffer<'a> {
    width: usize,
    height: usize,
    pitch: usize,
    buf: &'a mut [u8],
}

impl Framebuffer<'_> {
    fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, color: Color) {
        for yd in y..y + h {
            let spot = x * 4 + yd * self.pitch;
            self.buf[spot..spot + 4 * w]
                .as_chunks_mut::<4>()
                .0
                .fill(color.as_arr());
        }
    }
}

#[no_mangle]
pub fn main(_args: CliArgs) {
    let mut info = framebuffer_info();

    println!("fb0 = {info:?}");
    println!("sleep 500ms");
    // wait 500ms before doing the animation thing
    nanosleep(500_000_000);

    let fbo = OpenOptions::new()
        .write(true)
        .open("/dev/fb0")
        .expect("failed to open /dev/fb0");
    let meta = fbo.metadata().expect("failed to read fb file metadata");

    let fbo_mapped = map_file(fbo.as_desc(), None, meta.len, 0).expect("failed to map the fb");

    let buf = unsafe { slice::from_raw_parts_mut(fbo_mapped.as_ptr() as *mut u8, meta.len) };
    let mut backbuf = vec![0u8; buf.len()];

    info.buf = &mut backbuf;

    drawing(info, buf);

    unmap_file(fbo.as_desc(), fbo_mapped, 0).expect("failed to unmap the fb");

    drop(fbo);

    println!("fbo file metadata: {meta:?}");
}
