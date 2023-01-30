use limine::{LimineKernelAddressRequest, LimineKernelAddressResponse};
use spin::Lazy;
use x86_64::{PhysAddr, VirtAddr};

//

pub fn phys_addr() -> PhysAddr {
    PhysAddr::new(KERNEL_ADDR.physical_base)
}

pub fn virt_addr() -> VirtAddr {
    VirtAddr::new(KERNEL_ADDR.virtual_base)
}

//

static KERNEL_ADDR: Lazy<&'static LimineKernelAddressResponse> = Lazy::new(|| {
    static REQ: LimineKernelAddressRequest = LimineKernelAddressRequest::new(0);
    REQ.get_response().get().unwrap()
});
