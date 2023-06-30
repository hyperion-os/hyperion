#![no_std]
#![no_main]

#[no_mangle]
pub fn _start() -> i64 {
    42
}

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
