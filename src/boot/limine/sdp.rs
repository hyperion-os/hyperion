use crate::acpi::sdp::SdpDescriptor;
use limine::LimineRsdpRequest;
use spin::Lazy;

//

pub fn sdp() -> SdpDescriptor {
    static RSDP_REQ: LimineRsdpRequest = LimineRsdpRequest::new(0);
    static SDP_DESC: Lazy<SdpDescriptor> = Lazy::new(|| {
        let rsdp = RSDP_REQ
            .get_response()
            .get()
            .and_then(|rsdp| rsdp.address.as_ptr())
            .expect("RSDP data should be readable");

        unsafe { SdpDescriptor::try_read_from(rsdp as _) }.expect("RSDP should be valid")
    });

    *SDP_DESC
}
