use spin::Lazy;
use x86_64::instructions::{
    interrupts::without_interrupts,
    port::{Port, PortWriteOnly},
};

//

pub static RTC: Lazy<Rtc> = Lazy::new(Rtc::new);

//

pub struct Rtc {}

//

impl Rtc {
    pub fn new() -> Self {
        without_interrupts(|| unsafe {
            Port::<u8>::new(0x70).write(0x8A);
            Port::<u8>::new(0x71).write(0x20);

            Port::<u8>::new(0x70).write(0x8B);
            let reg_b = Port::<u8>::new(0x71).read();
            Port::<u8>::new(0x70).write(0x8B);
            Port::<u8>::new(0x71).write(reg_b | 0x40);
        });
        Self {}
    }
}

impl Default for Rtc {
    fn default() -> Self {
        Self::new()
    }
}
