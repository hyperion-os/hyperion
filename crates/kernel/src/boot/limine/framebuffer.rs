use core::slice;

use hyperion_boot_interface::framebuffer::FramebufferCreateInfo;
use limine::LimineFramebufferRequest;

//

pub fn framebuffer() -> Option<FramebufferCreateInfo> {
    static FB_REQ: LimineFramebufferRequest = LimineFramebufferRequest::new(0);

    FB_REQ
        .get_response()
        .get()
        .into_iter()
        .flat_map(|resp| resp.framebuffers().iter())
        .filter(|fb| fb.bpp == 32)
        .find_map(|fb| {
            let buf = unsafe { slice::from_raw_parts_mut(fb.address.as_ptr()?, fb.size()) };

            Some(FramebufferCreateInfo {
                buf,
                width: fb.width as _,
                height: fb.height as _,
                pitch: fb.pitch as _,
            })
        })
}
