use crate::{
    println,
    video::framebuffer::{get_fbo, Framebuffer, FramebufferInfo, FBO},
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
        });

    if let Some(mut fbo) = fbo {
        fbo.clear();
        FBO.call_once(|| Mutex::new(fbo));
    }
    println!("Global framebuffer: {:#?}", get_fbo().map(|f| f.info))
}
