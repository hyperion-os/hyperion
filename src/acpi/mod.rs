use crate::util::stack_str::StackStr;
use core::{mem, slice};

//

pub mod rsdp;
pub mod rsdt;

//

pub fn init() {
    _ = rsdp::RsdpDescriptor::get();
    _ = rsdt::Rsdt::get();
}

/// checksum_validation
///
/// sums up every byte in the structure
///
/// # Safety
///
/// * `size` has to be `None` or the memory range must be readable
unsafe fn bytes_sum_to_zero<T: Sized>(value: &T, size: Option<usize>) -> bool {
    let size = size.unwrap_or(mem::size_of::<T>());
    let bytes: &[u8] = unsafe { slice::from_raw_parts(value as *const T as *const u8, size) };

    bytes.iter().fold(0u8, |acc, v| acc.overflowing_add(*v).0) == 0
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpiOem {
    Bochs,
    Other(StackStr<6>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AcpiVersion {
    V1 = 1,
    V2 = 2,
}

//

impl From<StackStr<6>> for AcpiOem {
    fn from(v: StackStr<6>) -> Self {
        match v.as_str() {
            "BOCHS " => Self::Bochs,
            _ => Self::Other(v),
        }
    }
}
