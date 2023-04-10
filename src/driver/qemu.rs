use core::fmt::{Arguments, Write};
use spin::{Lazy, Mutex};
use uart_16550::SerialPort;
use x86_64::instructions::interrupts::without_interrupts;

//

#[doc(hidden)]
pub fn _print(args: Arguments) {
    without_interrupts(|| _ = COM1.lock().write_fmt(args))
}

//

static COM1: Lazy<Mutex<SerialPort>> = Lazy::new(|| {
    let mut port = unsafe { SerialPort::new(0x3f8) };
    port.init();
    Mutex::new(port)
});
