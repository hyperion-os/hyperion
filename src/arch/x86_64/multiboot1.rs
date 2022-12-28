use core::ffi::CStr;

#[allow(unused)]
#[repr(C)]
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
#[link_section = ".multiboot"]
static MULTIBOOT1_HEADER: Multiboot1Header = Multiboot1Header {
    magic: MAGIC,
    flags: FLAGS,
    checksum: (0x100000000 - (MAGIC + FLAGS) as u64) as u32,

    _unused: [0; 5],

    mode_type: 0, // 0 = linear graphics
    width: 0,     // 0 = no preference
    height: 0,    // 0 = no preference
    depth: 0,     // 0 = no preference
};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct Multiboot1Information {
    flags: u32,
    optional: [u8; 112],
}

impl Multiboot1Information {
    fn bootloader_name(&self) -> Option<&str> {
        if (self.flags & 1 << 9) != 0 {
            let ptr = u32::from_le_bytes((self.optional[60..=64]).try_into().ok()?) as _;
            let s = unsafe { CStr::from_ptr(ptr) };
            let s = s.to_str().ok()?;

            Some(s)
        } else {
            None
        }
    }

    fn framebuffer(&self) -> Option<&[u8]> {
        if (self.flags & 1 << 12) != 0 {
            Some(&self.optional[84..])
        } else {
            None
        }
    }
}

#[no_mangle]
extern "C" fn kernel_main(magic_num: u64) {
    let mb1_info_pointer = magic_num & u32::MAX as u64;
    let mb1_info = unsafe { *(mb1_info_pointer as *const Multiboot1Information) };

    crate::println!(
        "\0{:?}\n{:#b}\n{:?}",
        mb1_info.bootloader_name(),
        mb1_info.flags,
        mb1_info.framebuffer(),
    );

    crate::kernel_main();
}
