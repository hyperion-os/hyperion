#![no_std]

//

pub use hyperion_driver_pic as pic;
pub use hyperion_driver_pit as pit;
pub use hyperion_driver_rtc as rtc;

//

pub fn lazy_install() {
    use pit::PIT;
    use rtc::RtcDevice;

    *hyperion_vfs::IO_DEVICES.lock() = || {
        hyperion_vfs::install_dev("/dev/rtc", RtcDevice);
        // hyperion_vfs::install_dev("/dev/hpet", HpetDevice);
    };

    *hyperion_clock::PICK_CLOCK_SOURCE.lock() = || {
        // TODO: more clocks
        // Some(&*HPET)
        Some(&*PIT)
    };
}
