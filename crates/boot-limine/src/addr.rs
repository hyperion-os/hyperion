use limine::{HhdmRequest, KernelAddressRequest};
use spin::Lazy;

use crate::memmap;

//

pub fn hhdm_offset() -> u64 {
    static HHDM_OFFSET: Lazy<u64> = Lazy::new(|| {
        static REQ: HhdmRequest = HhdmRequest::new(0);
        REQ.get_response()
            .get()
            .expect("Cannot get LimineHHDM response")
            .offset
    });

    *HHDM_OFFSET
}

pub fn phys_addr() -> usize {
    KERNEL_ADDR.phys
}

pub fn virt_addr() -> usize {
    KERNEL_ADDR.virt
}

pub fn size() -> usize {
    KERNEL_ADDR.size
}

//

struct KernelAddress {
    phys: usize,
    virt: usize,
    size: usize,
}

//

static KERNEL_ADDR: Lazy<KernelAddress> = Lazy::new(|| {
    // FIXME: UB if initialized after bootloader reserved memory is freed
    static REQ: KernelAddressRequest = KernelAddressRequest::new(0);
    let resp = REQ
        .get_response()
        .get()
        .expect("Cannot get LimineHHDM response");

    let mut memmap_iter = memmap().filter(|map| map.is_kernel_and_modules());
    let kernel_map = memmap_iter.next().unwrap();
    assert_eq!(memmap_iter.next(), None);
    let size = kernel_map.len;

    KernelAddress {
        phys: resp.physical_base as _,
        virt: resp.virtual_base as _,
        size,
    }
});
