use crate::arch::done;
use core::panic::PanicInfo;

//

#[cfg(not(feature = "tests"))]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    crate::println!("Kernel {info}");
    done();
}

#[cfg(feature = "tests")]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    crate::testfw::test_panic_handler(info);
    done();
}
