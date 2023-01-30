use crate::{boot, debug};
use core::fmt::{self, Display, Formatter};

//

// pub static STORAGE: Once<Vec<ThreadLocal>> = Once::new();

//

pub fn init() -> ! {
    debug!("Waking up non-boot CPUs");
    boot::smp_init();
    crate::smp_main(Cpu {
        processor_id: 0,
        local_apic_id: 0,
    })
}

//

#[derive(Debug)]
pub struct Cpu {
    pub processor_id: u32,
    pub local_apic_id: u32,
}

impl Cpu {
    pub fn new(processor_id: u32, local_apic_id: u32) -> Self {
        Self {
            processor_id,
            local_apic_id,
        }
    }
}

impl Display for Cpu {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "CPU{}", self.processor_id)
    }
}

// pub struct ThreadLocal {
//     id: u64,
// }
