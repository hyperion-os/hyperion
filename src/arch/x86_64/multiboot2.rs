#[allow(unused)]
#[repr(C)]
pub struct Multiboot2Header {
    magic: u32,
    architecture: u32,
    header_length: u32,
    checksum: u32,
    end_tag_0: u32,
    end_tag_1: u32,
}

const MAGIC: u32 = 0xE85250D6;
const ARCH: u32 = 0; // 32 bit (protected mode)
const LEN: u32 = core::mem::size_of::<Multiboot2Header>() as u32;
const CHECKSUM: u32 = (0x100000000 - (MAGIC + ARCH + LEN) as u64) as u32;

#[used]
#[no_mangle]
#[link_section = ".multiboot"]
pub static MULTIBOOT2_HEADER: Multiboot2Header = Multiboot2Header {
    magic: MAGIC,
    architecture: ARCH,
    header_length: LEN,
    checksum: CHECKSUM,
    end_tag_0: 0,
    end_tag_1: 8,
};

#[no_mangle]
pub extern "C" fn kernel_main(_magic_num: u64) {
    crate::kernel_main();
}
