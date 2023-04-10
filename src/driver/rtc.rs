use crate::debug;
use spin::Lazy;
use x86_64::instructions::port::Port;

//

pub static RTC: Lazy<Rtc> = Lazy::new(Rtc::new);

//

pub struct Rtc {}

//

impl Rtc {
    pub fn new() -> Self {
        unsafe {
            Port::<u8>::new(0x70).write(0x8A);
            Port::<u8>::new(0x71).write(0x20);

            Port::<u8>::new(0x70).write(0x8B);
            let reg_b = Port::<u8>::new(0x71).read();
            Port::<u8>::new(0x70).write(0x8B);
            Port::<u8>::new(0x71).write(reg_b | 0x40);

            Port::<u8>::new(0x70).write(0x89);
            let year = Port::<u8>::new(0x71).read();
            debug!("year {year}");
        };
        debug!("RTC enabled");
        Self {}
    }

    pub fn read(&self) {
        while self.update_in_progress_flag() {}
    }

    fn update_in_progress_flag(&self) -> bool {
        unsafe {
            Port::<u8>::new(0x70).write(0x0A);
            Port::<u8>::new(0x71).read() & 0x80 != 0
        }
    }
}

impl Default for Rtc {
    fn default() -> Self {
        Self::new()
    }
}
