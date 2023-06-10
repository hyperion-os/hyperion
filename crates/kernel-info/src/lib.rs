#![no_std]

//

use hyperion_macros::{build_rev, build_time};

//

pub static NAME: &str = if cfg!(test) {
    "Hyperion-Testing"
} else {
    "Hyperion"
};

pub static VERSION: &str = env!("CARGO_PKG_VERSION");

// ofc. every kernel has to have this cringy ascii name splash
pub static SPLASH: &str = include_str!("../../../asset/splash");

pub static BUILD_TIME: &str = build_time!();

pub static BUILD_REV: &str = build_rev!();
