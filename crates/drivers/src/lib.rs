#![no_std]

//

pub use hyperion_driver_acpi as acpi;
pub use hyperion_driver_framebuffer as fbo;
// pub use hyperion_driver_pic as pic;
// pub use hyperion_driver_pit as pit;
pub use hyperion_driver_rtc as rtc;
use hyperion_sync as sync;
use hyperion_vfs::tree::IntoNode;

//

pub fn lazy_install_early(root: impl IntoNode) {
    if !sync::once!() {
        return;
    }

    let root = root.into_node();
    root.install_dev("dev/rtc", rtc::RtcDevice);
    root.install_dev("dev/hpet", acpi::hpet::HpetDevice);
    root.install_dev("dev/fb0", fbo::FboDevice::new());
    root.install_dev("dev/fb0-info", fbo::FboInfoDevice::new());

    hyperion_clock::set_source_picker(|| {
        // TODO: more clocks
        Some(&*acpi::hpet::HPET)
        // Some(&*pit::PIT)
    });

    hyperion_keyboard::force_init_queue();
    hyperion_driver_ps2::keyboard::init();
    hyperion_driver_ps2::mouse::init();
}

pub fn lazy_install_late() {}
