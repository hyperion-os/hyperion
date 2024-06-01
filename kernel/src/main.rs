#![no_std]
#![no_main]

//

use loader_info::LoaderInfo;
use log::println;
use riscv64_util::halt_and_catch_fire;
use spin::Mutex;
use util::rle::SegmentType;

use core::{fmt, panic::PanicInfo};

//

pub struct Syscon {
    _p: (),
}

impl Syscon {
    pub const unsafe fn init() -> Self {
        Self { _p: () }
    }

    pub fn poweroff(&mut self) -> ! {
        unsafe { Self::base().write_volatile(0x5555) };
        halt_and_catch_fire();
    }

    pub fn reboot(&mut self) -> ! {
        unsafe { Self::base().write_volatile(0x7777) };
        halt_and_catch_fire();
    }

    const fn base() -> *mut u32 {
        // TODO: get the address from devicetree
        0x10_0000 as _
    }
}

//

pub static SYSCON: Mutex<Syscon> = Mutex::new(unsafe { Syscon::init() });

//

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    halt_and_catch_fire();
}

#[no_mangle]
#[link_section = ".text.boot"]
extern "C" fn _start(this: usize, info: *const LoaderInfo) -> ! {
    assert_eq!(this, _start as _);

    uart_16550::install_logger();
    println!("hello from kernel");

    let info = unsafe { *info };
    println!("{info:#x?}");
    let memory = unsafe { &*info.memory };
    println!("{memory:#x?}");

    let total_usable_memory = memory
        .iter()
        .filter(|s| s.ty == SegmentType::Usable)
        .map(|s| s.size.get())
        .sum::<usize>();

    println!("total system memory = {total_usable_memory}");

    println!("done");
    SYSCON.lock().poweroff();
}
