use crate::{
    debug,
    video::framebuffer::{Framebuffer, FramebufferInfo},
};
use core::slice;
use limine::LimineFramebufferRequest;

//

pub fn framebuffer() -> Option<Framebuffer> {
    static FB_REQ: LimineFramebufferRequest = LimineFramebufferRequest::new(0);

    FB_REQ
        .get_response()
        .get()
        .into_iter()
        .flat_map(|resp| resp.framebuffers().iter())
        .find_map(|fb| {
            if fb.bpp != 32 {
                return None;
            }

            let buf = unsafe { slice::from_raw_parts_mut(fb.address.as_ptr()?, fb.size()) };

            Some(Framebuffer {
                buf,
                info: FramebufferInfo {
                    width: fb.width as _,
                    height: fb.height as _,
                    pitch: fb.pitch as _,
                },
            })
        })
}
