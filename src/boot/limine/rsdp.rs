use crate::acpi::rsdp::RsdpDescriptor;
use limine::LimineRsdpRequest;
use spin::Lazy;

//

pub fn rsdp() -> RsdpDescriptor {
    static RSDP_REQ: LimineRsdpRequest = LimineRsdpRequest::new(0);
    static RSDP_DESC: Lazy<RsdpDescriptor> = Lazy::new(|| {
        let rsdp = RSDP_REQ
            .get_response()
            .get()
            .and_then(|rsdp| rsdp.address.as_ptr())
            .expect("RSDP data should be readable");

        unsafe { RsdpDescriptor::try_read_from(rsdp as _) }.expect("RSDP should be valid")
    });

    *RSDP_DESC
}

//
