use core::fmt::{Arguments, Write};
use spin::{Lazy, Mutex};
use uart_16550::SerialPort;

//

static COM1: Lazy<Mutex<SerialPort>> = Lazy::new(|| {
    let mut port = unsafe { SerialPort::new(0x3f8) };
    port.init();
    Mutex::new(port)
});

//

#[doc(hidden)]
pub fn _print(args: Arguments) {
    let mut writer = COM1.lock();
    writer.write_fmt(args).unwrap();
}
