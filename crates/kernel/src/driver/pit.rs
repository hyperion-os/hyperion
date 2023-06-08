use spin::{Lazy, Mutex};
use x86_64::instructions::port::Port;

use super::pic::PICS;

//

pub static PIT: Lazy<Mutex<Pit>> = Lazy::new(|| {
    // dependencies
    Lazy::force(&PICS);

    Mutex::new(Pit::new())
});

// static PIT_CLOCK: AtomicUsize = AtomicUsize::new(0);

// 1193181.666...
const PIT_HZ_NUMERATOR: u32 = 1193182;

//

pub struct Pit {
    ch: [Port<u8>; 3],
    cmd: Port<u8>,
    delay: Port<u8>, // ?
    ch2_gate: Port<u8>,
}

//

impl Pit {
    pub const fn new() -> Self {
        Self {
            ch: [Port::new(0x40), Port::new(0x41), Port::new(0x42)],
            cmd: Port::new(0x43),
            delay: Port::new(0x60),
            ch2_gate: Port::new(0x61),
        }
    }

    pub fn _apic_simple_pit_wait(&mut self, micro_seconds: u32, pre: impl FnOnce()) {
        let divisor = PIT_HZ_NUMERATOR / (1_000_000 / micro_seconds);
        if divisor > 0x10000 {
            panic!("sleep time too long");
        }

        unsafe {
            // speaker channel 2 => controlled by PIT
            let gv = self.ch2_gate.read() & 0xFD;
            self.ch2_gate.write(gv | 0x1);

            // one shot cmd
            self.cmd.write(0b10110010);

            // write lower byte
            self.ch[2].write(divisor as u8);
            // wait for ack
            self.iowait();
            // write higher byte
            self.ch[2].write((divisor >> 8) as u8);

            pre();

            let gv = self.ch2_gate.read() & 0xFE;
            self.ch2_gate.write(gv);
            self.ch2_gate.write(gv | 0x1);

            // waiting has started
            while self.ch2_gate.read() & 0x20 != 0 {}
        }
    }

    fn iowait(&mut self) {
        unsafe { _ = self.delay.read() }
    }

    /* pub fn init(&mut self) {
        let freq = 3579545 / 3;

        if freq < 18 {
            return 0x10000;
        }

        if freq > 1193181 {
            return 1;
        }



        let x = (unsafe { self.ch2_gate.read() } & 0xfd) | 1;
    } */
}
