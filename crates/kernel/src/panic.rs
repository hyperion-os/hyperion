use core::panic::PanicInfo;

//

#[cfg(not(test))]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    hyperion_log::error!("Kernel CPU {info}");
    // hyperion_backtrace::print_backtrace();

    if hyperion_scheduler::running() {
        hyperion_scheduler::done();
    } else {
        hyperion_arch::die();
    }
}

#[cfg(test)]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    crate::testfw::test_panic_handler(info);
}
