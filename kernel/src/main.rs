#![no_std]
#![no_main]
#![feature(naked_functions)]

extern crate alloc;

//

use alloc::vec::Vec;
use loader_info::LoaderInfo;
use log::println;
use riscv64_util::{halt_and_catch_fire, PhysAddr};
use riscv64_vmm::PageTable;
use syscon::Syscon;
use util::{prefix::NumberFmt, rle::RleMemoryRef};

use core::{arch::asm, panic::PanicInfo};

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

    &kalloc::KALLOC;

    let uart = PhysAddr::new_truncate(0x1000_0000).to_hhdm().as_ptr_mut();
    unsafe { uart_16550::install_logger(uart) };
    println!("hello from kernel");

    let info = unsafe { *info };
    println!("{info:#x?}");
    let memory = RleMemoryRef::from_slice(unsafe { &*info.memory });
    println!("{memory:#x?}");

    // let fdt = unsafe {
    //     devicetree::Fdt::read(
    //         PhysAddr::from_phys_ptr(info.device_tree_blob)
    //             .to_hhdm()
    //             .as_ptr(),
    //     )
    // }
    // .expect("invalid device tree");
    // fdt.structure_parser().print_tree(0);

    let total_usable_memory = memory.iter_usable().map(|s| s.size.get()).sum::<usize>();

    println!("my address    = {:#x}", entry as usize);
    println!("usable memory = {}B", total_usable_memory.binary());

    mem::frame_alloc::init(memory);

    let frame = mem::frame_alloc::alloc();

    println!("page map = {}", PageTable::get_active().to_hhdm());

    let page_map: &mut PageTable = unsafe { &mut *PageTable::get_active().to_hhdm().as_ptr_mut() };
    println!(
        "{:?} == {:?}",
        frame.addr(),
        page_map.walk(frame.addr().to_hhdm())
    );

    scheduler::spawn(async {
        println!("hello from async");
    });

    scheduler::run_forever();

    println!("done, poweroff");
    Syscon::poweroff();
}
