#![no_std]

//

use core::fmt;

use spin::{Lazy, Mutex};

//

pub struct Uart {
    _p: (),
}

impl Uart {
    /// # Safety
    /// only one [`Uart`] should exist
    ///
    /// ns16550a compatible UART should be mapped to 0x1000_0000
    pub unsafe fn init() -> Self {
        let base = Self::base();

        unsafe {
            // data size to 2^0b11=2^3=8 bits -> IER interrupt enable
            base.add(3).write_volatile(0b11);
            // enable FIFO                    -> FCR FIFO control
            base.add(2).write_volatile(0b1);
            // enable interrupts              -> LCR line control
            base.add(1).write_volatile(0b1);

            // TODO (HARDWARE): real UART
        }

        Self { _p: () }
    }

    pub fn write(&mut self, byte: u8) {
        unsafe { Self::base().write_volatile(byte) };
    }

    pub fn read(&mut self) -> Option<u8> {
        let base = Self::base();

        // anything to read? <- LSR line status
        let avail = unsafe { base.add(5).read_volatile() } & 0b1 != 0;
        // let avail = false;

        if avail {
            Some(unsafe { base.read_volatile() })
        } else {
            None
        }
    }

    const fn base() -> *mut u8 {
        0x1000_0000 as _
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

pub fn install_logger() {
    static LOG: Lazy<UartLog> = Lazy::new(|| UartLog(Mutex::new(unsafe { Uart::init() })));

    struct UartLog(Mutex<Uart>);

    impl log::Logger for UartLog {
        fn print(&self, args: fmt::Arguments) {
            use core::fmt::Write;
            _ = self.0.lock().write_fmt(args);
        }
    }

    log::init_logger(&*LOG);
}
