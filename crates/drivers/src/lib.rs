#![no_std]
#![feature(maybe_uninit_slice)]

//

// pub use hyperion_driver_acpi as acpi;
// pub use hyperion_driver_framebuffer as fbo;
// pub use hyperion_driver_pic as pic;
// pub use hyperion_driver_pit as pit;
// pub use hyperion_driver_rtc as rtc;
use hyperion_sync as sync;

//

extern crate alloc;

//

// pub mod hpet;
// pub mod input;
pub mod log;
pub mod null;
pub mod rand;

//

pub fn lazy_install_early() {
    if !sync::once!() {
        return;
    }

    // let root = root.into_node().find("dev", true).unwrap();
    // root.install_dev("null", null::Null);
    // root.install_dev("random", rand::Random); // TODO: /dev/random is supposed to block when it runs out of entropy
    // root.install_dev("urandom", rand::Random);
    // root.install_dev("log", log::KernelLogs);
    // root.install_dev("rtc", rtc::RtcDevice);
    // root.install_dev("hpet", hpet::HpetDevice);
    // root.install_dev("fb0", fbo::FboDevice::new());
    // root.install_dev("fb0-info", fbo::FboInfoDevice::new());

    // root.install_dev("keyboard", input::KeyboardDevice);
    // root.install_dev("mouse", input::MouseDevice);

    // hyperion_clock::set_source_picker(|| {
    //     // TODO: more clocks
    //     Some(&*acpi::hpet::HPET)
    //     // Some(&*pit::PIT)
    // });

    // hyperion_driver_ps2::keyboard::init();
    // hyperion_driver_ps2::mouse::init();
}

pub fn lazy_install_late() {}
