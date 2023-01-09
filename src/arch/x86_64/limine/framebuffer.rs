use crate::{
    println,
    video::framebuffer::{Framebuffer, FBO},
};
use core::slice;
use limine::LimineFramebufferRequest;
use spin::Mutex;

//

pub fn init() {
    static FB_REQ: LimineFramebufferRequest = LimineFramebufferRequest::new(0);

    let fbo = FB_REQ
        .get_response()
        .get()
        .into_iter()
        .flat_map(|resp| resp.framebuffers().into_iter())
        .find_map(|fb| {
            if fb.bpp != 32 {
                return None;
            }

            let buf = unsafe { slice::from_raw_parts_mut(fb.address.as_ptr()?, fb.size()) };
            Some(Framebuffer {
                buf,
                width: fb.width as _,
                height: fb.height as _,
                pitch: fb.pitch as _,
            })
        });

    if let Some(fbo) = fbo {
        FBO.call_once(|| Mutex::new(fbo));
    }
    println!("Global framebuffer {:#?}", FBO.get())
}
