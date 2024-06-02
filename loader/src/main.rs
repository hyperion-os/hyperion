#![no_std]
#![no_main]
#![feature(naked_functions, format_args_nl, generic_nonzero)]

//! hyperion kernel loader for RISC-V
//!
//! this loads the baked in kernel ELF into higher half virtual memory, and then jumps there

//

use core::{arch::asm, ffi, num::NonZero, panic::PanicInfo, str};

use loader_info::LoaderInfo;
use log::println;
use syscon::Syscon;
use uart_16550::Uart;
use util::rle::{Region, RleMemory};
use xmas_elf::ElfFile;

use riscv64_vmm::{PageFlags, PageTable, VirtAddr};

//

extern "C" {
    static _kernel_beg: ffi::c_void;
    static _kernel_end: ffi::c_void;

    static _trampoline_beg: ffi::c_void;
    static _trampoline_end: ffi::c_void;
}

#[no_mangle]
#[link_section = ".text.boot"]
unsafe extern "C" fn _start() -> ! {
    unsafe {
        asm!(
            // init global pointer
            ".option push",
            ".option norelax",
            "la gp, _global_pointer",
            ".option pop",

            // a0 == 0x80200000 (but isn't for some reason?)
            // a1 == DTB (device tree binary)

            // init stack
            "la sp, _boot_stack_top",

            // call rust code
            "tail {entry}",
            entry = sym entry,
            options(noreturn)
        );
    }
}

extern "C" fn entry(_a0: usize, a1: usize) -> ! {
    uart_16550::install_logger();

    // FIXME: at least try the standard addresses for SYSCON and UART,
    // instead of just panicing after failing to parse the devicetree
    let tree = unsafe { devicetree::Fdt::read(a1 as _) }.expect("Devicetree is invalid");

    let mut memory = tree.usable_memory();

    let dtb_bottom = VirtAddr::new(a1).align_down();
    let dtb_top = VirtAddr::new(a1 + tree.header.totalsize as usize).align_up();
    let dtb_size = dtb_top.as_usize() - dtb_bottom.as_usize();
    memory.remove(Region {
        addr: dtb_bottom.as_usize(),
        size: dtb_size.try_into().unwrap(),
    });

    // reserve the kernel memory from the usable memory
    let kernel_beg = unsafe { &_kernel_beg } as *const _ as usize;
    let kernel_end = unsafe { &_kernel_end } as *const _ as usize;
    memory.remove(Region {
        addr: kernel_beg,
        size: NonZero::new(kernel_end - kernel_beg).unwrap(),
    });

    let page_table = PageTable::alloc_page_table(&mut memory);

    println!("map hardware");
    // syscon
    page_table.map_identity(
        &mut memory,
        VirtAddr::new(Syscon::base() as usize)..VirtAddr::new(Syscon::base() as usize + 0x1000),
        PageFlags::R | PageFlags::W,
    );
    // uart
    page_table.map_identity(
        &mut memory,
        VirtAddr::new(Uart::base() as usize)..VirtAddr::new(Uart::base() as usize + 0x1000),
        PageFlags::R | PageFlags::W,
    );

    println!("map loader");
    // only .text.trampoline needs to be mapped tho
    page_table.map_identity(
        &mut memory,
        VirtAddr::new(kernel_beg)..VirtAddr::new(kernel_end),
        PageFlags::R | PageFlags::W | PageFlags::X,
    );

    static KERNEL_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL"));
    let kernel_elf = ElfFile::new(KERNEL_ELF).expect("invalid ELF");
    println!("load kernel");
    load_kernel(&kernel_elf, page_table, &mut memory).expect("failed to load the kernel");

    let entry = kernel_elf.header.pt2.entry_point();
    assert_eq!(entry, 0xffffffff80000000);

    println!("enabling paging");
    unsafe { PageTable::activate(page_table as _) };

    let loader_info = LoaderInfo {
        device_tree_blob: a1 as _,
        memory: memory.as_slice(),
    };

    println!("far jump to kernel");
    unsafe { enter(entry as usize, &loader_info as *const _ as usize) };

    // println!("done, poweroff");
    // SYSCON.lock().poweroff();
}

#[link_section = ".text.trampoline"]
#[no_mangle]
#[naked]
unsafe extern "C" fn enter(
    entry: usize,       /* VirtAddr */
    loader_info: usize, /* LoaderInfo */
) -> ! {
    unsafe {
        asm!(
            // reset stack
            "la sp, _boot_stack_top",
            // far jump (no-return) to `entry` without saving the return address (x0)
            "jalr x0, 0(a0)",
            options(noreturn)
        )
    }
}

fn load_kernel(
    kernel_elf: &ElfFile,
    table: &mut PageTable,
    memory: &mut RleMemory,
) -> Result<(), &'static str> {
    for program in kernel_elf.program_iter() {
        if let xmas_elf::program::Type::Load = program.get_type()? {
            let virt_beg = VirtAddr::new_truncate(program.virtual_addr() as usize);
            let virt_end = virt_beg + program.mem_size() as usize;

            let file_beg = program.offset() as usize;
            let file_end = file_beg + program.file_size() as usize;

            let mut page_flags = PageFlags::empty();
            if program.flags().is_read() {
                page_flags |= PageFlags::R;
            }
            if program.flags().is_write() {
                page_flags |= PageFlags::W;
            }
            if program.flags().is_execute() {
                page_flags |= PageFlags::X;
            }

            let data = &kernel_elf.input[file_beg..file_end];
            table.map(memory, virt_beg..virt_end, page_flags, data);
        }
    }

    Ok(())
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{info}");
    riscv64_util::halt_and_catch_fire();
}
