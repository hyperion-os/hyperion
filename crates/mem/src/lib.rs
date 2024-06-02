#![no_std]
#![feature(maybe_uninit_slice)]

//

use core::{mem::MaybeUninit, ptr};

use log::println;
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

    println!("placing bitmap at = {:#x}", bitmap_region.addr);
    let bitmap =
        ptr::slice_from_raw_parts_mut(bitmap_region.addr as *mut MaybeUninit<u8>, bitmap_size);
    let bitmap = unsafe { &mut *bitmap };
    bitmap.fill(MaybeUninit::new(0));
    let bitmap = unsafe { MaybeUninit::slice_assume_init_mut(bitmap) };

    println!("bitmap initialized");
    static BITMAP: Once<Mutex<&mut [u8]>> = Once::new();
    BITMAP.call_once(|| Mutex::new(bitmap));

    // TODO: this will be replaced with the original hyperion PMM

    // usable_memory
}
