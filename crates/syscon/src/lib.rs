#![no_std]

//

use riscv64_util::halt_and_catch_fire;

//

pub struct Syscon {
    _p: (),
}

impl Syscon {
    // pub const unsafe fn init() -> Self {
    //     Self { _p: () }
    // }

    pub fn poweroff() -> ! {
        unsafe { Self::base().write_volatile(0x5555) };
        halt_and_catch_fire();
    }

    pub fn reboot() -> ! {
        unsafe { Self::base().write_volatile(0x7777) };
        halt_and_catch_fire();
    }

    pub const fn base() -> *mut u32 {
        // TODO: get the address from devicetree
        0x10_0000 as _
    }
}
