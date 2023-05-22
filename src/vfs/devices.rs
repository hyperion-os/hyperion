use crate::{
    driver::{acpi::hpet::HpetDevice, rtc::RtcDevice},
    vfs,
};

use super::Node;

//

pub fn install(root: Node) {
    vfs::install_dev_with(root.clone(), "/dev/rtc", RtcDevice);
    vfs::install_dev_with(root, "/dev/hpet", HpetDevice);
}
