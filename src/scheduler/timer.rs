use crate::driver::acpi::apic::ApicId;

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
