//! https://www.gnu.org/software/grub/manual/multiboot/multiboot.html

use core::{ffi::CStr, mem::transmute, slice};

use spin::Lazy;
use uart_16550::SerialPort;
use volatile::Volatile;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::println;

//

#[allow(unused)]
#[repr(packed)]
struct Multiboot1Header {
    magic: u32,
    flags: u32,
    checksum: u32,

    _unused: [u32; 5], // header_addr, load_addr, load_end_addr, bss_end_addr, entry_addr

    mode_type: u32,
    width: u32,
    height: u32,
    depth: u32,
}

const MAGIC: u32 = 0x1BADB002;
const ALIGN: u32 = 1 << 0;
const MEMINFO: u32 = 1 << 1;
const VIDEO: u32 = 1 << 2;
const FLAGS: u32 = ALIGN | MEMINFO | VIDEO;

#[used]
#[no_mangle]
#[link_section = ".boot"]
static MULTIBOOT1_HEADER: Multiboot1Header = Multiboot1Header {
    magic: MAGIC,
    flags: FLAGS,
    checksum: (0x100000000 - (MAGIC + FLAGS) as u64) as u32,

    _unused: [0; 5],

    mode_type: 0, // 0 = linear graphics
    width: 1280,  // 0 = no preference
    height: 720,  // 0 = no preference
    depth: 32,    // 0 = no preference
};

#[derive(Debug, Clone, Copy)]
#[repr(packed)]
struct Multiboot1Information {
    flags: u32,

    mem_lower: u32,
    mem_upper: u32,

    boot_device: u32,

    cmdline: u32,

    mods_count: u32,
    mods_addr: u32,

    syms: [u32; 4],
    mmap_len: u32,
    mmap_addr: u32,

    drives_len: u32,
    drives_addr: u32,

    config_table: u32,

    boot_loader_name: u32,

    apm_table: u32,

    // VESA Bios Extensions table
    vbe_control_info: u32,
    vbe_mode_info: u32,
    vbe_mode: u16,
    vbe_interface_seg: u16,
    vbe_interface_off: u16,
    vbe_interface_len: u16,

    // Framebuffer table
    framebuffer_addr: u64,
    framebuffer_pitch: u32,
    framebuffer_width: u32,
    framebuffer_height: u32,
    framebuffer_bpp: u8,
    framebuffer_type: u8,
    color_info: [u8; 6],
}

#[allow(unused)]
const fn test() {
    unsafe {
        // at compile time: make sure that Multiboot1Information is exactly 116 bytes
        transmute::<[u8; 116], Multiboot1Information>([0; 116]);
    }
}

#[derive(Debug)]
pub struct BootInfo {
    pub cmdline: Option<&'static str>,
    pub bootloader: Option<&'static str>,
    pub framebuffer: Option<Framebuffer>,
}

pub struct Framebuffer {
    pub buffer: &'static mut [u8],
    pub stride: u32, // bytes per row
    pub width: u32,
    pub height: u32,
}

impl core::fmt::Debug for Framebuffer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Framebuffer")
            .field("buffer", &self.buffer.as_ptr_range())
            .field("stride", &self.stride)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl Multiboot1Information {
    fn build(&self) -> BootInfo {
        BootInfo {
            cmdline: self.cmdline(),
            bootloader: self.bootloader(),
            framebuffer: self.framebuffer(),
        }
    }

    fn cmdline(&self) -> Option<&'static str> {
        if self.get_bit(2) {
            // SAFETY: if flags[2] is set, this pointer is valid
            let s = unsafe { CStr::from_ptr(self.cmdline as _) };
            let s = s.to_str().ok()?;

            Some(s)
        } else {
            None
        }
    }

    fn bootloader(&self) -> Option<&'static str> {
        if self.get_bit(9) {
            // SAFETY: if flags[9] is set, this pointer is valid
            let s = unsafe { CStr::from_ptr(self.boot_loader_name as _) };
            let s = s.to_str().ok()?;

            Some(s)
        } else {
            None
        }
    }

    fn vbe(&self) -> Option<impl core::fmt::Debug> {
        if self.get_bit(11) {
            let vbe = (
                self.vbe_control_info,
                self.vbe_mode_info,
                self.vbe_mode,
                self.vbe_interface_seg,
                self.vbe_interface_off,
                self.vbe_interface_len,
            );
            crate::println!("{vbe:?}");
            Some(vbe)
        } else {
            None
        }
    }

    fn framebuffer(&self) -> Option<Framebuffer> {
        if self.get_bit(12) {
            let size = self.framebuffer_pitch as usize * self.framebuffer_height as usize;
            let buffer =
                unsafe { slice::from_raw_parts_mut(self.framebuffer_addr as *mut _, size) };

            Some(Framebuffer {
                buffer,
                stride: self.framebuffer_pitch,
                width: self.framebuffer_width,
                height: self.framebuffer_height,
            })
        } else {
            None
        }
    }

    fn get_bit(&self, n: u8) -> bool {
        (self.flags & 1 << n) != 0
    }
}

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint);
    idt
});

#[no_mangle]
extern "C" fn _start_rust(magic_num: u64) -> ! {
    *crate::BOOTLOADER.lock() = "Multiboot1";
    crate::print!("\0");
    // let mb1_info_pointer = magic_num & u32::MAX as u64;
    // let mb1_info = unsafe { *(mb1_info_pointer as *const Multiboot1Information) };
    // let mut boot_info = mb1_info.build();

    // crate::println!("{boot_info:#?} {mb1_info:#?}");

    crate::println!("test");

    // IDT.load();
    // x86_64::instructions::interrupts::int3();
    crate::println!("comp");

    // if let Some(fb) = &mut boot_info.framebuffer {
    //     // fb.buffer[1000] = 255;
    //     // fb.buffer.fill(255);
    // }

    crate::kernel_main();
}

extern "x86-interrupt" fn breakpoint(stack: InterruptStackFrame) {
    // println!("{stack:?}");
}
