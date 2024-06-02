#![no_std]
#![no_main]

//

use loader_info::LoaderInfo;
use log::println;
use riscv64_util::halt_and_catch_fire;
use syscon::Syscon;
use util::rle::SegmentType;

use core::panic::PanicInfo;

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

    println!("done, poweroff");
    Syscon::poweroff();
}
