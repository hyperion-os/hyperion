#![no_std]
#![feature(maybe_uninit_slice, maybe_uninit_fill)]

//

use hyperion_futures::lazy::Once;
// pub use hyperion_driver_acpi as acpi;
// pub use hyperion_driver_framebuffer as fbo;
// pub use hyperion_driver_pic as pic;
// pub use hyperion_driver_pit as pit;
// pub use hyperion_driver_rtc as rtc;

//

extern crate alloc;

//

// pub mod hpet;
// pub mod input;
pub mod log;
pub mod null;
pub mod rand;

//

pub async fn lazy_install() {
    static VFS_INIT: Once<()> = Once::new();
    VFS_INIT.call_once(lazy_install_inner()).await;
}

async fn lazy_install_inner() {
    hyperion_vfs::bind(None, "/dev/null", null::DEV_NULL.clone())
        .await
        .unwrap();
    hyperion_vfs::bind(None, "/dev/log", log::DEV_LOG.clone())
        .await
        .unwrap();
    hyperion_vfs::bind(None, "/dev/random", rand::DEV_RANDOM.clone())
        .await
        .unwrap();
    hyperion_vfs::bind(None, "/dev/urandom", rand::DEV_RANDOM.clone())
        .await
        .unwrap();

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
