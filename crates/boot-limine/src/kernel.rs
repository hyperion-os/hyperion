use core::slice;

use limine::KernelFileRequest;

//

pub(crate) static REQ: KernelFileRequest = KernelFileRequest::new(0);

pub fn kernel_file() -> Option<&'static [u8]> {
    REQ.get_response()
        .get()
        .and_then(|resp| resp.kernel_file.get())
        .and_then(|file| {
            Some(unsafe { slice::from_raw_parts(file.base.as_ptr()?, file.length as _) })
        })
}
