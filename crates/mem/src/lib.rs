#![no_std]
#![feature(maybe_uninit_slice)]

//

use core::{mem::MaybeUninit, ptr};

use log::println;
use riscv64_vmm::PhysAddr;
use spin::{Mutex, Once};
use util::rle::RleMemoryRef;

//

pub fn init_frame_allocator(memory: &RleMemoryRef) {
    println!("bitmap allocator minimum = {:#x}", memory.min_usable_addr());
    println!("bitmap allocator maximum = {:#x}", memory.max_usable_addr());

    let bitmap_size = (memory.max_usable_addr() - memory.min_usable_addr()).div_ceil(8);
    let bitmap_region = memory
        .iter_usable()
        .find(|usable| usable.size.get() >= bitmap_size)
        .expect("not enough contiguous memory for the bitmap allocator");

    let bitmap_addr = PhysAddr::new(bitmap_region.addr).to_higher_half();

    println!("placing bitmap at = {bitmap_addr}");
    let bitmap: *mut [MaybeUninit<u8>] =
        ptr::slice_from_raw_parts_mut(bitmap_addr.as_ptr_mut(), bitmap_size);
    let bitmap: &mut [MaybeUninit<u8>] = unsafe { &mut *bitmap };

    println!("zero init bitmap");
    bitmap.fill(MaybeUninit::new(0));
    let bitmap = unsafe { MaybeUninit::slice_assume_init_mut(bitmap) };

    println!("bitmap initialized");
    static BITMAP: Once<Mutex<&mut [u8]>> = Once::new();
    BITMAP.call_once(|| Mutex::new(bitmap));

    // TODO: this will be replaced with the original hyperion PMM

    // usable_memory
}
