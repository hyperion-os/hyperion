use core::fmt::{Arguments, Write};

use spin::{Lazy, Mutex};
use uart_16550::SerialPort;
use x86_64::instructions::interrupts::without_interrupts;

//

#[doc(hidden)]
pub fn _print(args: Arguments) {
    without_interrupts(|| _ = COM1.lock().write_fmt(args))
}

/* /// Force unlock this [`Mutex`].
///
/// # Safety
///
/// This is *extremely* unsafe if the lock is not held by the current
/// thread. However, this can be useful in some instances for exposing the
/// lock to FFI that doesn't know how to deal with RAII.
pub unsafe fn force_unlock() {
    COM1.force_unlock();
} */

//

static COM1: Lazy<Mutex<SerialPort>> = Lazy::new(|| {
    let mut port = unsafe { SerialPort::new(0x3f8) };
    port.init();
    Mutex::new(port)
});
