use crate::{
    driver::{acpi::hpet::HpetDevice, rtc::RtcDevice},
    vfs,
};

//

pub fn install() {
    vfs::install_dev("/dev/rtc", RtcDevice);
    vfs::install_dev("/dev/hpet", HpetDevice);
}
