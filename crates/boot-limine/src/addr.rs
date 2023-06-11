use limine::{LimineHhdmRequest, LimineKernelAddressRequest, LimineKernelAddressResponse};
use spin::Lazy;

//

pub fn phys_addr() -> usize {
    KERNEL_ADDR.physical_base as _
}

pub fn virt_addr() -> usize {
    KERNEL_ADDR.virtual_base as _
}

pub fn hhdm_offset() -> u64 {
    static HHDM_OFFSET: Lazy<u64> = Lazy::new(|| {
        static REQ: LimineHhdmRequest = LimineHhdmRequest::new(0);
        REQ.get_response()
            .get()
            .expect("Cannot get Limine HHDM response")
            .offset
    });

    *HHDM_OFFSET
}

//

static KERNEL_ADDR: Lazy<&'static LimineKernelAddressResponse> = Lazy::new(|| {
    static REQ: LimineKernelAddressRequest = LimineKernelAddressRequest::new(0);
    REQ.get_response()
        .get()
        .expect("Cannot get Limine HHDM response")
});
