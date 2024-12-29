#![no_std]
#![feature(slice_as_chunks)]

//

extern crate alloc;

use alloc::{string::String, vec};
use core::slice;

use glam::{Mat4, Vec3, Vec3Swizzles, Vec4, Vec4Swizzles};
use hyperion_color::Color;
use libstd::{fs::OpenOptions, io::BufReader, println, sys::*};

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

    const COLOR_BG: Color = Color::from_hex("#222222").unwrap();
    const COLOR_BLOCK: Color = Color::from_hex("#00FFFF").unwrap();

    fb.fill(0, 0, w, h, COLOR_BG);

    let mid_x = fb.width as i32 / 2;
    let mid_y = fb.height as i32 / 2;
    let mut a = 0.0f32;

    for i in 0..1000 {
        let i = i as f32 * 0.05;
        let cos = libm::cosf(i) * 25.0;
        let sin = libm::sinf(i) * 25.0;

        let points = [(cos, sin), (-cos, -sin), (sin, -cos), (-sin, cos)]
            .map(|(x, y)| ((50.0 + x) as usize, (50.0 + y) as usize));

        // draw
        for (x, y) in points {
            fb.fill(x, y, 20, 20, COLOR_BLOCK);
        }

        let red = Mat4::from_rotation_y(a);
        let blue = Mat4::from_rotation_y(a * 2.0);
        fb.draw_cube(mid_x, mid_y, red, 100.0, Color::RED);
        fb.draw_cube(mid_x, mid_y, blue, 80.0, Color::BLUE);

        // wait
        buf.copy_from_slice(fb.buf);
        nanosleep(5_000_000);

        // clear
        let red = Mat4::from_rotation_y(a);
        let blue = Mat4::from_rotation_y(a * 2.0);
        fb.draw_cube(mid_x, mid_y, red, 100.0, COLOR_BG);
        fb.draw_cube(mid_x, mid_y, blue, 80.0, COLOR_BG);
        a += 0.01;

        for (x, y) in points {
            fb.fill(x, y, 20, 20, COLOR_BG);
        }
    }

    fb.fill(0, 0, w, h, Color::BLACK);
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
    fn pixel(&mut self, x: usize, y: usize, color: Color) {
        let spot = x * 4 + y * self.pitch;
        self.buf[spot..spot + 4].copy_from_slice(&color.as_arr()[..]);
    }

    fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, color: Color) {
        for yd in y..y + h {
            let spot = x * 4 + yd * self.pitch;
            self.buf[spot..spot + 4 * w]
                .as_chunks_mut::<4>()
                .0
                .fill(color.as_arr());
        }
    }

    fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let dx = x0.abs_diff(x1);
        let dy = y0.abs_diff(y1);

        if dx > dy {
            for x in x0.min(x1)..=x0.max(x1) {
                let t = (x - x0) as f32 / (x1 - x0) as f32;
                let y = (t * (y1 - y0) as f32) as i32 + y0;

                self.pixel(x as _, y as _, color);
            }
        } else {
            for y in y0.min(y1)..=y0.max(y1) {
                let t = (y - y0) as f32 / (y1 - y0) as f32;
                let x = (t * (x1 - x0) as f32) as i32 + x0;

                self.pixel(x as _, y as _, color);
            }
        }
    }

    fn draw_cube(&mut self, x: i32, y: i32, model: Mat4, s: f32, color: Color) {
        let mat = Mat4::perspective_rh(0.005, 1.0, 0.01, 600.0)
            * Mat4::look_at_rh(Vec3::new(0.0, 0.0, 300.0), Vec3::ZERO, Vec3::NEG_Y)
            * model;

        let mut translated_line = |a: Vec3, b: Vec3| {
            let a = mat * Vec4::from((a, 1.0));
            let b = mat * Vec4::from((b, 1.0));
            let a = (a.xyz() / a.w).xy().as_ivec2();
            let b = (b.xyz() / b.w).xy().as_ivec2();

            self.draw_line(a.x + x, a.y + y, b.x + x, b.y + y, color);
        };

        translated_line(Vec3::new(-s, -s, -s), Vec3::new(s, -s, -s));
        translated_line(Vec3::new(-s, s, -s), Vec3::new(s, s, -s));
        translated_line(Vec3::new(-s, -s, s), Vec3::new(s, -s, s));
        translated_line(Vec3::new(-s, s, s), Vec3::new(s, s, s));

        translated_line(Vec3::new(-s, -s, -s), Vec3::new(-s, s, -s));
        translated_line(Vec3::new(s, -s, -s), Vec3::new(s, s, -s));
        translated_line(Vec3::new(-s, -s, s), Vec3::new(-s, s, s));
        translated_line(Vec3::new(s, -s, s), Vec3::new(s, s, s));

        translated_line(Vec3::new(-s, -s, -s), Vec3::new(-s, -s, s));
        translated_line(Vec3::new(s, -s, -s), Vec3::new(s, -s, s));
        translated_line(Vec3::new(-s, s, -s), Vec3::new(-s, s, s));
        translated_line(Vec3::new(s, s, -s), Vec3::new(s, s, s));
    }
}

pub fn main() {
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
