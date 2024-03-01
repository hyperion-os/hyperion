use hyperion_color::Color;
use hyperion_syscall::timestamp;
use hyperion_windowing::global::{GlobalFb, Region};

use crate::{CURSOR, WINDOWS};

//

#[derive(Debug, Clone, Copy)]
struct Rect {
    top_left: (usize, usize),
    bottom_right: (usize, usize),
}

impl Rect {
    fn new(x: usize, y: usize, w: usize, h: usize) -> Self {
        Self {
            top_left: (x, y),
            bottom_right: (x + w, y + h),
        }
    }

    fn union(&self, other: &Self) -> Self {
        Self {
            top_left: (
                self.top_left.0.min(other.top_left.0),
                self.top_left.1.min(other.top_left.1),
            ),
            bottom_right: (
                self.bottom_right.0.max(other.bottom_right.0),
                self.bottom_right.1.max(other.bottom_right.1),
            ),
        }
    }
}

//

pub fn blitter() {
    let mut global_fb = GlobalFb::lock_global_fb();
    let mut global_fb = global_fb.as_region();

    let mut backbuf = vec![0u32; global_fb.width * global_fb.height];
    let mut backbuf = unsafe {
        Region::new(
            backbuf.as_mut_ptr(),
            global_fb.width,
            global_fb.width,
            global_fb.height,
        )
    };

    // background
    let bg_col = Color::from_hex("#141414").unwrap().as_u32();
    backbuf.volatile_fill(0, 0, usize::MAX, usize::MAX, bg_col);
    global_fb.volatile_copy_from(&backbuf, 0, 0);

    // // cursor icon
    // let mut cursor_icon = [0u32; 16 * 16];
    // for y in 0..16 {
    //     for x in 0..16 {
    //         cursor_icon[x + y * 16] = if x > y || (x * x) + (y * y) > 15 * 15 {
    //             Color::BLACK
    //         } else {
    //             Color::WHITE
    //         }
    //         .as_u32();
    //     }
    // }
    // let mut cursor_icon = unsafe { Region::new(cursor_icon.as_mut_ptr(), 16, 16, 16) };

    let mut old_cursor = (0, 0);

    let mut next_sync = timestamp().unwrap() as u64;
    loop {
        let mut dirty = Rect::new(0, 0, 0, 0);

        let mut windows = WINDOWS.lock().unwrap();

        // remove all windows
        for window in windows.iter_mut() {
            dirty = dirty.union(&Rect::new(
                window.old_info.x,
                window.old_info.y,
                window.old_info.w,
                window.old_info.h,
            ));
            backbuf.volatile_fill(
                window.old_info.x,
                window.old_info.y,
                window.old_info.w,
                window.old_info.h,
                bg_col,
            );
            window.old_info = window.info;
        }
        // remove the cursor
        dirty = dirty.union(&Rect::new(old_cursor.0, old_cursor.1, 16, 16));
        backbuf.volatile_fill(old_cursor.0, old_cursor.1, 16, 16, bg_col);
        // blit all windows
        for window in windows.iter() {
            dirty = dirty.union(&Rect::new(
                window.info.x,
                window.info.y,
                window.info.w,
                window.info.h,
            ));
            let fb = unsafe {
                Region::new(
                    window.shmem_ptr.as_ptr(),
                    window.info.w,
                    window.info.w,
                    window.info.h,
                )
            };
            backbuf.volatile_copy_from(&fb, window.info.x as isize, window.info.y as isize);
        }
        drop(windows);
        // blit cursor
        let (m_x, m_y) = CURSOR.load();
        let cursor = (m_x as usize, m_y as usize);
        old_cursor = cursor;
        dirty = dirty.union(&Rect::new(cursor.0, cursor.1, 16, 16));
        for yo in 0..16usize {
            for xo in 0..16usize {
                if !(xo > yo || (xo * xo) + (yo * yo) >= 15 * 15) {
                    backbuf.volatile_fill(
                        cursor.0 + xo,
                        cursor.1 + yo,
                        1,
                        1,
                        Color::WHITE.as_u32(),
                    );
                }
            }
        }

        // update
        let dirty_backbuf = backbuf.subregion(
            dirty.top_left.0,
            dirty.top_left.1,
            dirty.bottom_right.0 - dirty.top_left.0,
            dirty.bottom_right.1 - dirty.top_left.1,
        );
        global_fb.volatile_copy_from(
            &dirty_backbuf,
            dirty.top_left.0 as isize,
            dirty.top_left.1 as isize,
        );

        // println!("VSYNC");
        next_sync += 16_666_667;
        hyperion_syscall::nanosleep_until(next_sync);
        // hyperion_syscall::yield_now();
    }
}

fn blit_chunk() {}
