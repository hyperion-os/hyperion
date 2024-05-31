#![no_std]
#![no_main]

//

use loader_info::LoaderInfo;

use core::panic::PanicInfo;

//

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
extern "C" fn _start(info: *const LoaderInfo) -> ! {
    let info = unsafe { *info };

    loop {}
}
