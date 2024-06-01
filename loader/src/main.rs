#![no_std]
#![no_main]
#![feature(naked_functions, format_args_nl, generic_nonzero)]

//! hyperion kernel loader for RISC-V
//!
//! this loads the baked in kernel ELF into higher half virtual memory, and then jumps there

//

mod logger;

//

use core::{arch::asm, fmt, num::NonZero, panic::PanicInfo, str};

use loader_info::LoaderInfo;
use log::println;
use spin::Mutex;
use util::rle::{Region, RleMemory};
use xmas_elf::ElfFile;

use riscv64_vmm::{PageFlags, PageTable, VirtAddr};

//

pub struct Uart {
    _p: (),
}

impl Uart {
    pub fn write(&mut self, byte: u8) {
        unsafe { Self::base().write_volatile(byte) };
    }

    pub fn read(&mut self) -> Option<u8> {
        let base = Self::base();

        // anything to read? <- LSR line status
        let avail = unsafe { base.add(5).read_volatile() } & 0b1 != 0;
        // let avail = false;

        if avail {
            Some(unsafe { base.read_volatile() })
        } else {
            None
        }
    }

    const fn new() -> Self {
        Self { _p: () }
    }

    fn init(&mut self) {
        let base = Self::base();

        unsafe {
            // data size to 2^0b11=2^3=8 bits -> IER interrupt enable
            base.add(3).write_volatile(0b11);
            // enable FIFO                    -> FCR FIFO control
            base.add(2).write_volatile(0b1);
            // enable interrupts              -> LCR line control
            base.add(1).write_volatile(0b1);

            // TODO (HARDWARE): real UART
        }
    }

    const fn base() -> *mut u8 {
        0x1000_0000 as _
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write(byte);
        }
        Ok(())
    }
}

//

pub struct Syscon {
    _p: (),
}

impl Syscon {
    pub fn poweroff(&mut self) -> ! {
        unsafe { Self::base().write_volatile(0x5555) };
        halt_and_catch_fire();
    }

    pub fn reboot(&mut self) -> ! {
        unsafe { Self::base().write_volatile(0x7777) };
        halt_and_catch_fire();
    }

    const fn init() -> Self {
        Self { _p: () }
    }

    const fn base() -> *mut u32 {
        0x10_0000 as _
    }
}

//

// pub static UART: Lazy<Mutex<Uart>> = Lazy::new(|| Mutex::new(Uart::init()));
pub static SYSCON: Mutex<Syscon> = Mutex::new(Syscon::init());

//

extern "C" {
    static _kernel_beg: ();
    static _kernel_end: ();

    static _trampoline_beg: ();
    static _trampoline_end: ();
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
    logger::init_logger();

    // FIXME: at least try the standard addresses for SYSCON and UART,
    // instead of just panicing after failing to parse the devicetree
    let mut tree = unsafe { devicetree::Fdt::read(a1 as _) }.expect("Devicetree is invalid");

    let mut memory = tree.usable_memory();

    memory.remove(Region {
        addr: a1,
        size: (tree.header.totalsize as usize).try_into().unwrap(),
    });

    // reserve the kernel memory from the usable memory
    let kernel_beg = unsafe { &_kernel_beg } as *const _ as usize;
    let kernel_end = unsafe { &_kernel_end } as *const _ as usize;
    memory.remove(Region {
        addr: kernel_beg,
        size: NonZero::new(kernel_end - kernel_beg).unwrap(),
    });

    println!("{memory:#x?}");

    // // test fill whole memory space with zeros
    // for usable in memory.iter_usable() {
    //     let region: *mut [MaybeUninit<u64>] =
    //         ptr::slice_from_raw_parts_mut(usable.addr as _, usable.size.get() / 8);
    //     let region = unsafe { &mut *region };

    //     for p in region {
    //         // println!("write {:x}", p as *const _ as usize);
    //         p.write(0);
    //     }
    // }

    // mem::init_frame_allocator(memory);

    // let x = unsafe { &_kernel_end } as *const () as *mut u8;
    // for i in 0.. {
    //     println!("probing {:#x}", x as usize + i * 0x1000);
    //     unsafe { x.add(i * 0x1000).write(5) };
    // }

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
    let kernel_elf = ElfFile::new(&KERNEL_ELF).expect("invalid ELF");
    println!("load kernel");
    load_kernel(&kernel_elf, page_table, &mut memory);

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
            "jalr x0, 0(a0)", // far jump (no-return) to `entry` without saving the return address (x0)
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

/// HCF instruction
fn halt_and_catch_fire() -> ! {
    loop {
        wait_for_interrupts();
    }
}

/// WFI instruction
extern "C" fn wait_for_interrupts() {
    unsafe {
        asm!("wfi");
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{info}");
    SYSCON.lock().poweroff();
}
