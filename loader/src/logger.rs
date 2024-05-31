use core::fmt;

use spin::Mutex;

use crate::Uart;

//

pub fn init_logger() {
    LOG.0.lock().init();
    log::init_logger(&LOG);
}

//

static LOG: UartLog = UartLog(Mutex::new(Uart::new()));

struct UartLog(Mutex<Uart>);

impl log::Logger for UartLog {
    fn print(&self, args: fmt::Arguments) {
        use core::fmt::Write;
        _ = self.0.lock().write_fmt(args);
    }
}
