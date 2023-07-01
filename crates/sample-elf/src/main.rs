#![no_std]
#![no_main]
#![feature(lang_items)]

#[no_mangle]
pub fn _start() -> i64 {
    42
}

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// for clippy:
#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
