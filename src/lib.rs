#![no_std]
#![no_main]

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[link_section = ".boot"]
pub extern "C" fn _start() {}
