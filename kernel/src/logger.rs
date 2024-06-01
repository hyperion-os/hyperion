use core::fmt;

use spin::Mutex;

use uart_16550::Uart;

//

pub fn init_logger() {
    static LOG: UartLog = UartLog(Mutex::new(Uart::new()));
    log::init_logger(&LOG);
}

//

struct UartLog(Mutex<Uart>);

impl log::Logger for UartLog {
    fn print(&self, args: fmt::Arguments) {
        use core::fmt::Write;
        _ = self.0.lock().write_fmt(args);
    }
}
