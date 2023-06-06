use crate::driver::acpi::hpet::HPET;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant {
    inner: u128,
}

//

impl Instant {
    pub fn now() -> Self {
        Self {
            inner: HPET.femtos(),
        }
    }
}
