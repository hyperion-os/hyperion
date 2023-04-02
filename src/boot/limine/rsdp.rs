use limine::LimineRsdpRequest;
use spin::Lazy;

//

pub fn rsdp() -> *const () {
    static RSDP_REQ: LimineRsdpRequest = LimineRsdpRequest::new(0);
    static RSDP_DESC: Lazy<usize> = Lazy::new(|| {
        let rsdp = RSDP_REQ
            .get_response()
            .get()
            .and_then(|rsdp| rsdp.address.as_ptr())
            .expect("RSDP data should be readable");

        rsdp as _
    });

    *RSDP_DESC as *const ()
}
