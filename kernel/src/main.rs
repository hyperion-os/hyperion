#![no_std]
#![no_main]
#![feature(naked_functions)]

//

use loader_info::LoaderInfo;
use log::println;
use riscv64_util::halt_and_catch_fire;
use syscon::Syscon;
use util::{postifx::NumberPostfix, rle::SegmentType};

use core::{arch::asm, panic::PanicInfo};

//

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    halt_and_catch_fire();
}

#[naked]
#[no_mangle]
#[link_section = ".text.boot"]
extern "C" fn _start(_this: usize, _info: *const LoaderInfo) -> ! {
    unsafe {
        asm!(
            // init global pointer
            ".option push",
            ".option norelax",
            "la gp, _global_pointer",
            ".option pop",

            // init stack
            "la sp, _boot_stack_top",

            // call rust code
            "tail {entry}",
            entry = sym entry,
            options(noreturn)
        );
    }
}

extern "C" fn entry(this: usize, info: *const LoaderInfo) -> ! {
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

    println!("my address    = {:#x}", entry as usize);
    println!("usable memory = {}B", total_usable_memory.postfix_binary());

    println!("done, poweroff");
    Syscon::poweroff();
}
