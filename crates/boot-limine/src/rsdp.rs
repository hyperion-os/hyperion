use limine::LimineRsdpRequest;
use spin::Lazy;

//

pub fn rsdp() -> Option<*const ()> {
    static RSDP_REQ: LimineRsdpRequest = LimineRsdpRequest::new(0);
    static RSDP_DESC: Lazy<Option<usize>> = Lazy::new(|| {
        RSDP_REQ
            .get_response()
            .get()
            .and_then(|rsdp| rsdp.address.as_ptr())
            .map(|ptr| ptr as usize)
    });

    (*RSDP_DESC).map(|ptr| ptr as *const ())
}
