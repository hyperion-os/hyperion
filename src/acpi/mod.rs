pub mod sdp;
pub mod sdt;

//

pub fn init() {
    _ = sdp::SdpDescriptor::get();
}
