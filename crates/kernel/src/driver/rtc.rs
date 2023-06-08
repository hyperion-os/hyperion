use core::{
    mem,
    sync::atomic::{AtomicU8, Ordering},
};

use chrono::{DateTime, TimeZone, Utc};
use spin::Mutex;
use x86_64::instructions::{interrupts::without_interrupts, port::Port};

use crate::{
    debug, error,
    util::slice_read::slice_read,
    vfs::{FileDevice, IoError, IoResult},
};

//

pub static RTC: Rtc = Rtc::new();
pub static RTC_CENTURY_REG: AtomicU8 = AtomicU8::new(0);
pub const CUR_YEAR: u32 = include!("./rtc.year");

//

pub struct Rtc {
    ports: Mutex<RtcPorts>,
    time: Time, // TODO: unix time stamp?
}

//

impl Rtc {
    pub const fn new() -> Self {
        Self {
            ports: Mutex::new(RtcPorts {
                cmos_addr: Port::new(0x70),
                cmos_data: Port::new(0x71),
            }),
            time: Time::new(),
        }
    }

    pub fn enable_ints(&self) {
        without_interrupts(|| {
            let mut ports = self.ports.lock();
            unsafe {
                ports.cmos_addr.write(0x8B);
                let prev = ports.cmos_data.read();
                ports.cmos_addr.write(0x8B);
                ports.cmos_data.write(prev | 0x40);
            }
        });
    }

    pub fn int_ack(&self) {
        let mut ports = self.ports.lock();
        unsafe {
            ports.cmos_addr.write(0x0C);
            // throw away
            _ = ports.cmos_data.read();
        }
    }

    pub fn init_clock(&self) {
        for _ in 0..100 {
            let Some(now) = self.now() else {
                continue
            };

            debug!("RTC time is {now}");

            // Self::now already stored it
            // self.time.store(now.timestamp_nanos());
            return;
        }

        // 01/01/2023 - 00:00:00;000;000;000
        const FALLBACK: i64 = 1_672_531_200_000_000_000;
        self.time.store(FALLBACK);
        error!("Failed to init system clock, RTC gave invalid times, fallback time set");
    }

    pub fn now(&self) -> Option<DateTime<Utc>> {
        let time = self.ports.lock().read();
        let time = Utc
            .with_ymd_and_hms(
                time.full_year as _,
                time.month as _,
                time.day as _,
                time.hour as _,
                time.min as _,
                time.sec as _,
            )
            .single();

        if let Some(time) = time {
            self.time.store(time.timestamp_nanos());
        }

        time
    }

    pub fn now_bytes(&self) -> [u8; 8] {
        _ = self.now();
        let timestamp = self.time.load();
        timestamp.to_le_bytes()
    }
}

pub struct RtcDevice;

impl FileDevice for RtcDevice {
    fn len(&self) -> usize {
        mem::size_of::<i64>()
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        let bytes = &RTC.now_bytes()[..];
        slice_read(bytes, offset, buf)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

impl Default for Rtc {
    fn default() -> Self {
        Self::new()
    }
}

//

#[cfg(target_has_atomic_load_store = "64")]
use core::sync::atomic::AtomicI64;

#[cfg(not(target_has_atomic_load_store = "64"))]
use spin::RwLock;

struct Time {
    #[cfg(target_has_atomic_load_store = "64")]
    store_a: AtomicI64,
    #[cfg(not(target_has_atomic_load_store = "64"))]
    store_b: RwLock<i64>, // TODO: ring buffer
}

#[derive(Debug, PartialEq, Eq)]
struct RtcTime {
    sec: u8,
    min: u8,
    hour: u8,
    day: u8,
    month: u8,
    year: u8,
    cent: Option<u8>,

    full_year: u32, // f u if you live in or after the year 4294967296
}

struct RtcPorts {
    cmos_addr: Port<u8>,
    cmos_data: Port<u8>,
}

//

impl Time {
    const fn new() -> Self {
        Self {
            #[cfg(target_has_atomic_load_store = "64")]
            store_a: AtomicI64::new(0),
            #[cfg(not(target_has_atomic_load_store = "64"))]
            store_b: RwLock::new(0),
        }
    }

    fn store(&self, val: i64) {
        #[cfg(target_has_atomic_load_store = "64")]
        {
            self.store_a.store(val, Ordering::SeqCst);
        }
        #[cfg(not(target_has_atomic_load_store = "64"))]
        {
            *self.store_b.write() = val;
        }
    }

    fn load(&self) -> i64 {
        #[cfg(target_has_atomic_load_store = "64")]
        {
            self.store_a.load(Ordering::SeqCst)
        }
        #[cfg(not(target_has_atomic_load_store = "64"))]
        {
            *self.store_b.read()
        }
    }
}

impl RtcPorts {
    // TODO: enable RTC IRQ to get more accurate time
    /* /// enable RTC IRQ
    pub fn enable() {
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
    } */

    pub fn read(&mut self) -> RtcTime {
        while self.update_in_progress_flag() {}

        // read registers until the same values come twice in a row
        // to avoid an update happening in the middle of reading them
        //
        // after 100 tries, give up and just return the value
        let mut last = self.get_regs();
        for _ in 0..100 {
            let now = self.get_regs();
            if now == last {
                break;
            }
            last = now;
        }

        let reg_b = self.get_reg(0x0B);

        // BCD to binary conversion
        if reg_b & 0x04 == 0 {
            let bcd_to_bin = |bcd: u8| -> u8 { (bcd & 0x0F) + bcd / 16 * 10 };
            last.sec = bcd_to_bin(last.sec);
            last.min = bcd_to_bin(last.sec);
            last.hour = (last.hour & 0x0F) + (((last.hour & 0x70) / 16 * 10) | (last.hour & 0x80));
            last.day = bcd_to_bin(last.day);
            last.month = bcd_to_bin(last.month);
            last.year = bcd_to_bin(last.year);
            last.cent = last.cent.map(bcd_to_bin);
        }

        // 12hr to 24hr (the superior time format)
        if reg_b & 0x02 == 0 && last.hour & 0x80 != 0 {
            last.hour = ((last.hour & 0x7F) + 12) % 24;
        }

        last.full_year = if let Some(cent) = last.cent {
            last.year as u32 + cent as u32 * 100
        } else {
            let mut year = last.year as u32 + (CUR_YEAR / 100) * 100;
            if year < CUR_YEAR {
                year += 100;
            }
            year
        };

        last
    }

    fn update_in_progress_flag(&mut self) -> bool {
        self.get_reg(0x0A) & 0x80 != 0
    }

    fn century(&mut self) -> u8 {
        RTC_CENTURY_REG.load(Ordering::SeqCst)
    }

    fn get_reg(&mut self, reg: u8) -> u8 {
        unsafe {
            self.cmos_addr.write(reg);
            self.cmos_data.read()
        }
    }

    fn get_regs(&mut self) -> RtcTime {
        let cent = self.century();
        RtcTime {
            sec: self.get_reg(0x00),
            min: self.get_reg(0x02),
            hour: self.get_reg(0x04),
            day: self.get_reg(0x07),
            month: self.get_reg(0x08),
            year: self.get_reg(0x09),
            cent: if cent != 0 {
                Some(self.get_reg(cent))
            } else {
                None
            },

            full_year: 0,
        }
    }
}
