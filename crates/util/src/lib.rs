#![no_std]
#![feature(generic_nonzero, const_option)]

//

pub mod prefix;
pub mod rle;

//

pub const fn align_up(addr: usize, align: usize) -> usize {
    assert!(align.is_power_of_two(), "align has to be a power of 2");
    let mask = align - 1;

    if addr & mask == 0 {
        addr
    } else {
        (addr | mask).checked_add(1).expect("align_up overflow")
    }
}

pub const fn align_down(addr: usize, align: usize) -> usize {
    assert!(align.is_power_of_two(), "align has to be a power of 2");
    let mask = align - 1;
    addr & !mask
}

pub const fn is_aligned(addr: usize, align: usize) -> bool {
    assert!(align.is_power_of_two(), "align has to be a power of 2");
    addr % align == 0
}
