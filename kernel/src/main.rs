#![no_std]
#![no_main]
#![feature(naked_functions)]

//

use loader_info::LoaderInfo;
use log::println;
use riscv64_util::halt_and_catch_fire;
use riscv64_vmm::PhysAddr;
use syscon::Syscon;
use util::{
    postifx::NumberPostfix,
    rle::{RleMemoryRef, SegmentType},
};

use core::{arch::asm, panic::PanicInfo, slice};

//

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    println!("{info}");
    halt_and_catch_fire();
}

#[naked]
#[no_mangle]
#[link_section = ".text.boot"]
extern "C" fn _start(_this: usize, _info: *const LoaderInfo) -> ! {
    unsafe {
        asm!(
            // init global pointer
            ".option push",
            ".option norelax",
            "la gp, _global_pointer",
            ".option pop",

            // init stack
            "la sp, _boot_stack_top",

            // call rust code
            "tail {entry}",
            entry = sym entry,
            options(noreturn)
        );
    }
}

extern "C" fn entry(this: usize, info: *const LoaderInfo) -> ! {
    assert_eq!(this, _start as _);

    let uart = PhysAddr::new_truncate(0x1000_0000)
        .to_higher_half()
        .as_ptr_mut();
    unsafe { uart_16550::install_logger(uart) };
    println!("hello from kernel");

    let info = unsafe { *info };
    println!("{info:#x?}");
    let memory = RleMemoryRef::from_slice(unsafe { &*info.memory });
    println!("{memory:#x?}");

    for usable in memory.iter_usable() {
        // println!("filling {:#x}", usable.addr);
        let usable = unsafe {
            slice::from_raw_parts_mut(
                PhysAddr::new(usable.addr)
                    .to_higher_half()
                    .as_ptr_mut::<[u64; 512]>(),
                usable.size.get() / core::mem::size_of::<[u64; 512]>(),
            )
        };

        for b in usable {
            println!("filling {:#x}", b as *mut _ as usize);
            *b = [0; 512];
        }
    }

    println!("memtest done");

    // let fdt =
    //     unsafe { devicetree::Fdt::read(info.device_tree_blob as _) }.expect("invalid device tree");
    // fdt.structure_parser().print_tree(0);

    let total_usable_memory = memory.iter_usable().map(|s| s.size.get()).sum::<usize>();

    println!("my address    = {:#x}", entry as usize);
    println!("usable memory = {}B", total_usable_memory.postfix_binary());

    mem::init_frame_allocator(memory);

    println!("done, poweroff");
    Syscon::poweroff();
}
