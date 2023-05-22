use alloc::collections::BinaryHeap;
use spin::Mutex;

use crate::{driver::acpi::apic::ApicId, util::atomic_map::AtomicMap};

//

/* pub static DEADLINES: BinaryHeap<TimerEntry>; */

//

/* pub struct TimerEntry {
    // deadline:
} */

//

pub fn test() {
    let lapic = ApicId::current().lapic_mut();
    lapic.regs().timer_current.read();
}
