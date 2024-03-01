use hyperion_color::Color;
use hyperion_syscall::timestamp;
use hyperion_windowing::global::{GlobalFb, Region};

use crate::{CURSOR, WINDOWS};

//

pub fn blitter() {
    let mut global_fb = GlobalFb::lock_global_fb();
    let mut global_fb = global_fb.as_region();

    // background
    global_fb.volatile_fill(
        0,
        0,
        usize::MAX,
        usize::MAX,
        Color::from_hex("#141414").unwrap().as_u32(),
    );

    let depth_buffer = global_fb.width * global_fb.height;

    let mut old_cursor = (0, 0);

    let mut next_sync = timestamp().unwrap() as u64;
    loop {
        // blit all windows
        let mut windows = WINDOWS.lock().unwrap();
        for (pixels, window) in windows.iter_mut().filter_map(|w| Some((w.shmem_ptr?, w))) {
            if window.old_info != window.info {
                // remove non overlapping parts of the old window
                // if window.info.x > window.old_info.x && window.info.y < window.old_info.y {
                //     global_fb.volatile_fill(
                //         window.old_info.x,
                //         window.info.y,
                //         window.info.x - window.old_info.x,
                //         window.old_info.y - window.info.y,
                //         Color::from_hex("#141414").unwrap().as_u32(),
                //     );
                // }
                if window.info.x > window.old_info.x {
                    global_fb.volatile_fill(
                        window.old_info.x,
                        window.old_info.y,
                        window.info.x - window.old_info.x,
                        window.old_info.h,
                        Color::from_hex("#141414").unwrap().as_u32(),
                    );
                }
                if window.old_info.x + window.old_info.w > window.info.x + window.info.w {
                    global_fb.volatile_fill(
                        window.info.x + window.info.w,
                        window.old_info.y,
                        window.old_info.x + window.old_info.w - window.info.x - window.info.w,
                        window.old_info.h,
                        Color::from_hex("#141414").unwrap().as_u32(),
                    );
                }
                if window.info.y > window.old_info.y {
                    global_fb.volatile_fill(
                        window.old_info.x,
                        window.old_info.y,
                        window.old_info.w,
                        window.info.y - window.old_info.y,
                        Color::from_hex("#141414").unwrap().as_u32(),
                    );
                }
                if window.old_info.y + window.old_info.h > window.info.y + window.info.h {
                    global_fb.volatile_fill(
                        window.old_info.x,
                        window.info.y + window.info.h,
                        window.old_info.w,
                        window.old_info.y + window.old_info.h - window.info.y - window.info.h,
                        Color::from_hex("#141414").unwrap().as_u32(),
                    );
                }
                // global_fb.volatile_fill(
                //     window.old_info.x,
                //     window.old_info.y,
                //     window.old_info.w,
                //     window.old_info.h,
                //     Color::from_hex("#141414").unwrap().as_u32(),
                // );
                window.old_info = window.info;
            }

            let fb = unsafe {
                Region::new(pixels.as_ptr(), window.info.w, window.info.w, window.info.h)
            };
            // TODO: smarter blitting to avoid copying every single window every single frame
            global_fb.volatile_copy_from(&fb, window.info.x as isize, window.info.y as isize);
        }
        drop(windows);

        // remove the old cursor
        global_fb.volatile_fill(
            old_cursor.0,
            old_cursor.1,
            16,
            16,
            Color::from_hex("#141414").unwrap().as_u32(),
        );

        // blit cursor
        let (m_x, m_y) = CURSOR.load();
        let (c_x, c_y) = (m_x as usize, m_y as usize);
        global_fb.volatile_fill(c_x, c_y, 16, 16, Color::WHITE.as_u32());
        old_cursor = (c_x, c_y);

        // println!("VSYNC");
        next_sync += 16_666_667;
        hyperion_syscall::nanosleep_until(next_sync);
        // hyperion_syscall::yield_now();
    }
}
