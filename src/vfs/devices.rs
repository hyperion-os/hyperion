use super::Node;
use crate::{
    driver::{acpi::hpet::HpetDevice, rtc::RtcDevice},
    vfs,
};

//

pub fn install(root: Node) {
    vfs::install_dev_with(root.clone(), "/dev/rtc", RtcDevice);
    vfs::install_dev_with(root, "/dev/hpet", HpetDevice);
}
