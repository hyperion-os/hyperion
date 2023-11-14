#![no_std]

//

use core::sync::atomic::{AtomicBool, Ordering};

pub use hyperion_driver_acpi as acpi;
pub use hyperion_driver_framebuffer as fbo;
// pub use hyperion_driver_pic as pic;
// pub use hyperion_driver_pit as pit;
pub use hyperion_driver_rtc as rtc;
use hyperion_vfs::tree::IntoNode;

//

pub fn lazy_install_early(root: impl IntoNode) {
    let root = root.into_node();

    static ONCE: AtomicBool = AtomicBool::new(true);
    if !ONCE.swap(false, Ordering::Relaxed) {
        return;
    }

    root.install_dev("dev/rtc", rtc::RtcDevice);
    root.install_dev("dev/hpet", acpi::hpet::HpetDevice);
    root.install_dev("dev/fbo", fbo::FboDevice);

    hyperion_clock::set_source_picker(|| {
        // TODO: more clocks
        Some(&*acpi::hpet::HPET)
        // Some(&*pit::PIT)
    });
}

pub fn lazy_install_late() {
    static ONCE: AtomicBool = AtomicBool::new(true);
    if !ONCE.swap(false, Ordering::Relaxed) {
        return;
    }

    hyperion_keyboard::force_init_queue();
    hyperion_driver_ps2::keyboard::init();
    hyperion_driver_ps2::mouse::init();
}
