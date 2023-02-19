use crate::arch::done;
use core::panic::PanicInfo;

//

#[cfg(not(test))]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    crate::println!("Kernel CPU {info}");
    done();
}

#[cfg(test)]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    crate::testfw::test_panic_handler(info);
    done();
}
