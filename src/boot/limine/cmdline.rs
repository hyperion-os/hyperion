use limine::LimineKernelFileRequest;

//

pub fn cmdline() -> Option<&'static str> {
    static REQ: LimineKernelFileRequest = LimineKernelFileRequest::new(0);
    REQ.get_response()
        .get()
        .and_then(|resp| resp.kernel_file.get())
        .and_then(|file| file.cmdline.to_str())
        .and_then(|cmdline| cmdline.to_str().ok())
}
