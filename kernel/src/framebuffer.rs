use bootloader_api::{
    info::{FrameBuffer, Optional},
    BootInfo,
};
use spin::Mutex;

//

pub fn init(boot_info: &'static mut BootInfo) {
    let mut framebuffer = Optional::None;
    core::mem::swap(&mut boot_info.framebuffer, &mut framebuffer);

    if let Some(framebuffer) = framebuffer.into_option() {
        init_with(framebuffer);
    }
}

pub fn init_with(framebuffer: FrameBuffer) {
    *FRAMEBUFFER.lock() = Some(framebuffer);
}

pub fn clear() {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(fb) = &mut *fb {
        fb.buffer_mut().fill(0);
    }
}

pub fn print_char(character: u8) {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(fb) = &mut *fb {
        let px = fb.info().bytes_per_pixel;
        let row = px * fb.info().stride;

        for y in 0..8 {
            for x in 0..8 {
                for channel in 0..4 {
                    fb.buffer_mut()[channel + x * px + y * row] = FONT[character as usize][y][x];
                }
            }
        }
    }
}

//

static FRAMEBUFFER: Mutex<Option<FrameBuffer>> = Mutex::new(None);

static FONT: [[[u8; 8]; 8]; 256] = {
    let mut font = [[[0; 8]; 8]; 256];
    font[b'H' as usize] = [
        [0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF],
        [0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF],
        [0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF],
        [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        [0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF],
        [0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF],
        [0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF],
    ];
    font
};
