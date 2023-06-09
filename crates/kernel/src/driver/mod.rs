use self::{
    acpi::hpet::{HpetDevice, HPET},
    rtc::RtcDevice,
};

//

pub mod acpi;
pub mod pic;
pub mod pit;
pub mod ps2;
pub mod rtc;

//

pub fn lazy_install() {
    *hyperion_vfs::IO_DEVICES.lock() = || {
        hyperion_vfs::install_dev("/dev/rtc", RtcDevice);
        hyperion_vfs::install_dev("/dev/hpet", HpetDevice);
    };

    *hyperion_clock::PICK_CLOCK_SOURCE.lock() = || {
        // TODO: more clocks
        Some(&*HPET)
    };
}
