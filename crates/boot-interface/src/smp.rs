use core::fmt::{self, Display, Formatter};



use crate::loader::boot;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cpu {
    pub processor_id: u32,
    pub local_apic_id: u32,
}

impl Cpu {
    pub fn new_boot() -> Self {
        boot().bsp()
    }

    pub const fn new(processor_id: u32, local_apic_id: u32) -> Self {
        Self {
            processor_id,
            local_apic_id,
        }
    }

    pub const fn is_boot(&self) -> bool {
        self.processor_id == 0
    }
}

impl Display for Cpu {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self {
            processor_id: p_id,
            local_apic_id: a_id,
        } = *self;
        if f.alternate() {
            write!(f, "CPU-{p_id} (APIC-{a_id})",)
        } else {
            write!(f, "CPU-{p_id}")
        }
    }
}

// pub struct ThreadLocal {
//     id: u64,
// }
