use core::ptr;

use hyperion_boot_interface::FramebufferCreateInfo;
use limine::FramebufferRequest;
use spin::Once;

//

pub fn framebuffer() -> Option<FramebufferCreateInfo> {
    static FB_INFO: Once<Option<FramebufferCreateInfo>> = Once::new();
    static FB_REQ: FramebufferRequest = FramebufferRequest::new(0);

    *FB_INFO.call_once(|| {
        FB_REQ
            .get_response()
            .get()
            .into_iter()
            .flat_map(|resp| resp.framebuffers().iter())
            .filter(|fb| fb.bpp == 32)
            .find_map(|fb| {
                let buf = ptr::slice_from_raw_parts_mut(fb.address.as_ptr()?, fb.size());
                Some(FramebufferCreateInfo {
                    buf,
                    width: fb.width as _,
                    height: fb.height as _,
                    pitch: fb.pitch as _,
                })
            })
    })
}

pub fn init_fb() {
    if let Some(mut fb) = framebuffer() {
        let buf = unsafe { fb.buf_mut() };

        buf.fill(0);
    }
}
