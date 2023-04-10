use core::fmt::{Arguments, Write};
use spin::{Lazy, Mutex};
use uart_16550::SerialPort;

//

#[doc(hidden)]
pub fn _print(args: Arguments) {
    _ = COM1.lock().write_fmt(args);
}

/// Unlocks the COM1 writer IF it is locked by this exact thread
///
/// # Safety
///
/// This function is extremely unsafe and should only be used by the double fault handler when the
/// same thread had locked this
pub unsafe fn unlock() {
    // TODO: SMP
    // if COM1_LOCKER.load(Ordering::SeqCst) != crate::THREAD {
    //     return;
    // }

    COM1.force_unlock()
}

//

// static COM1_LOCKER: AtomicUsize = AtomicUsize::new(0);
static COM1: Lazy<Mutex<SerialPort>> = Lazy::new(|| {
    let mut port = unsafe { SerialPort::new(0x3f8) };
    port.init();
    Mutex::new(port)
});
