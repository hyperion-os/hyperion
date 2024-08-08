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

    let memmap_iter = memmap().filter(|map| map.is_kernel_and_modules());

    let mut min = usize::MAX;
    let mut max = usize::MIN;

    for kernel_map in memmap_iter {
        min = min.min(kernel_map.base);
        max = max.max(kernel_map.base + kernel_map.len);
    }

    if max <= min {
        panic!("kernel memmap missing");
    }

    if !(min..=max).contains(&(resp.physical_base as usize)) {
        panic!("kernel is not in kernel memmap");
    }

    KernelAddress {
        phys: min,
        virt: resp.virtual_base as usize - (resp.physical_base as usize - min),
        size: max - min,
    }
});
