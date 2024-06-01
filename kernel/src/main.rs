#![no_std]
#![no_main]

//

use loader_info::LoaderInfo;
use log::println;
use util::rle::SegmentType;

use core::{fmt, panic::PanicInfo};

//

mod logger;

//

pub struct Uart {
    _p: (),
}

impl Uart {
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

    const fn new() -> Self {
        Self { _p: () }
    }

    fn init(&mut self) {
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

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[link_section = ".text.boot"]
extern "C" fn _start(this: usize, info: *const LoaderInfo) -> ! {
    assert_eq!(this, _start as _);

    logger::init_logger();
    println!("hello from kernel");

    let info = unsafe { *info };
    println!("{info:#x?}");
    let memory = unsafe { &*info.memory };
    println!("{memory:#x?}");

    let total_usable_memory = memory
        .iter()
        .filter(|s| s.ty == SegmentType::Usable)
        .map(|s| s.size.get())
        .sum::<usize>();

    println!("total system memory = {total_usable_memory}");

    loop {}
}
