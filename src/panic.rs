use crate::{
    arch::{done, int},
    backtrace,
};
use core::panic::PanicInfo;

//

#[cfg(not(test))]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    int::disable();
    panic_unwind(info);
    done();
}

#[cfg(test)]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    int::disable();
    panic_unwind(info);
    crate::testfw::test_panic_handler(info);
    done();
}

fn panic_unwind(info: &PanicInfo) {
    crate::println!("Kernel CPU {info} {}", info.can_unwind());
    backtrace::print_backtrace();
}
