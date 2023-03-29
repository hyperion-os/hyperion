pub mod rsdp;

//

pub fn init() {
    _ = rsdp::RsdpDescriptor::get();
}
