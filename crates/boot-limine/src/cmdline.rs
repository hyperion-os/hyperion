use super::kernel::REQ;

//

pub fn cmdline() -> Option<&'static str> {
    REQ.get_response()
        .get()
        .and_then(|resp| resp.kernel_file.get())
        .and_then(|file| file.cmdline.to_str())
        .and_then(|cmdline| cmdline.to_str().ok())
}
