#![no_std]

//

use core::sync::atomic::{AtomicBool, Ordering};

pub use hyperion_driver_acpi as acpi;
pub use hyperion_driver_framebuffer as fbo;
pub use hyperion_driver_pic as pic;
pub use hyperion_driver_pit as pit;
pub use hyperion_driver_rtc as rtc;

//

pub fn lazy_install_early() {
    static ONCE: AtomicBool = AtomicBool::new(true);
    if !ONCE.swap(false, Ordering::Relaxed) {
        return;
    }

    *hyperion_vfs::IO_DEVICES.lock() = || {
        hyperion_vfs::install_dev("/dev/rtc", rtc::RtcDevice);
        hyperion_vfs::install_dev("/dev/hpet", acpi::hpet::HpetDevice);
        hyperion_vfs::install_dev("/dev/fbo", fbo::FboDevice);
    };

    *hyperion_clock::PICK_CLOCK_SOURCE.lock() = || {
        // TODO: more clocks
        Some(&*acpi::hpet::HPET)
        // Some(&*pit::PIT)
    };
}

pub fn lazy_install_late() {
    static ONCE: AtomicBool = AtomicBool::new(true);
    if !ONCE.swap(false, Ordering::Relaxed) {
        return;
    }

    hyperion_driver_ps2::keyboard::init();
    hyperion_driver_ps2::mouse::init();
}
