#![no_std]

//

use core::fmt;

use spin::{Lazy, Mutex, Once};

//

pub struct Uart {
    base: *mut u8,
}

unsafe impl Send for Uart {}

impl Uart {
    /// # Safety
    /// only one [`Uart`] should exist
    ///
    /// ns16550a compatible UART should be mapped to `base`
    pub unsafe fn init(base: *mut u8) -> Self {
        unsafe {
            // data size to 2^0b11=2^3=8 bits -> IER interrupt enable
            base.add(3).write_volatile(0b11);
            // enable FIFO                    -> FCR FIFO control
            base.add(2).write_volatile(0b1);
            // enable interrupts              -> LCR line control
            base.add(1).write_volatile(0b1);

            // TODO (HARDWARE): real UART
        }

        Self { base }
    }

    pub fn write(&mut self, byte: u8) {
        unsafe { self.base.write_volatile(byte) };
    }

    pub fn read(&mut self) -> Option<u8> {
        // anything to read? <- LSR line status
        let avail = unsafe { self.base.add(5).read_volatile() } & 0b1 != 0;
        // let avail = false;

        if avail {
            Some(unsafe { self.base.read_volatile() })
        } else {
            None
        }
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write(byte);
        }
        Ok(())
    }
}

//

/// # Safety
/// see [`Uart::init`]
pub unsafe fn install_logger(uart_addr: *mut u8) {
    static UART: Once<UartLog> = Once::new();
    log::init_logger(UART.call_once(|| UartLog(Mutex::new(unsafe { Uart::init(uart_addr) }))));

    struct UartLog(Mutex<Uart>);

    impl log::Logger for UartLog {
        fn print(&self, args: fmt::Arguments) {
            use core::fmt::Write;
            _ = self.0.lock().write_fmt(args);
        }
    }
}
