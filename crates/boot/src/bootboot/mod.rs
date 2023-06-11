#[allow(unused)]
const BOOTBOOT_MAGIC: &'static [u8; 5usize] = b"BOOT\0";

#[allow(unused)]
const BOOTBOOT_MMIO: u64 = 0xfffffffff8000000; /* memory mapped IO virtual address */
#[allow(unused)]
const BOOTBOOT_FB: u64 = 0xfffffffffc000000; /* frame buffer virtual address */
#[allow(unused)]
const BOOTBOOT_INFO: u64 = 0xffffffffffe00000; /* bootboot struct virtual address */
#[allow(unused)]
const BOOTBOOT_ENV: u64 = 0xffffffffffe01000; /* environment string virtual address */
#[allow(unused)]
const BOOTBOOT_CORE: u64 = 0xffffffffffe02000; /* core loadable segment start */

#[allow(unused)]
const PROTOCOL_MINIMAL: u32 = 0;
#[allow(unused)]
const PROTOCOL_STATIC: u32 = 1;
#[allow(unused)]
const PROTOCOL_DYNAMIC: u32 = 2;
#[allow(unused)]
const PROTOCOL_BIGENDIAN: u32 = 0x80;

#[allow(unused)]
const LOADER_BIOS: u32 = 0 << 2;
#[allow(unused)]
const LOADER_UEFI: u32 = 1 << 2;
#[allow(unused)]
const LOADER_RPI: u32 = 2 << 2;
#[allow(unused)]
const LOADER_COREBOOT: u32 = 3 << 2;

#[allow(unused)]
const FB_ARGB: u32 = 0;
#[allow(unused)]
const FB_RGBA: u32 = 1;
#[allow(unused)]
const FB_ABGR: u32 = 2;
#[allow(unused)]
const FB_BGRA: u32 = 3;

#[allow(unused)]
const MMAP_USED: u32 = 0; /* don't use. Reserved or unknown regions */
#[allow(unused)]
const MMAP_FREE: u32 = 1; /* usable memory */
#[allow(unused)]
const MMAP_ACPI: u32 = 2; /* acpi memory, volatile and non-volatile as well */
#[allow(unused)]
const MMAP_MMIO: u32 = 3; /* memory mapped IO region */

#[allow(unused)]
const INITRD_MAXSIZE: u32 = 16; /* Mb */

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct MMapEnt {
    ptr: u64,
    size: u64,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct BootBoot {
    /* first 64 bytes is platform independent */
    magic: [u8; 4usize],    /* 'BOOT' magic */
    size: u32,              /* length of bootboot structure, minimum 128 */
    protocol: u8,           /* 1, static addresses, see PROTOCOL_* and LOADER_* above */
    fb_type: u8,            /* framebuffer type, see FB_* above */
    numcores: u16,          /* number of processor cores */
    bspid: u16,             /* Bootsrap processor ID (Local APIC Id on x86_64) */
    timezone: i16,          /* in minutes -1440..1440 */
    datetime: [u8; 8usize], /* in BCD yyyymmddhhiiss UTC (independent to timezone) */

    initrd_ptr: u64, /* ramdisk image position and size */
    initrd_size: u64,

    fb_ptr: *mut u8, /* framebuffer pointer and dimensions */
    fb_size: u32,
    fb_width: u32,
    fb_height: u32,
    fb_scanline: u32,

    arch: Arch,

    mmap: MMapEnt,
}

#[derive(Clone, Copy)]
#[repr(C)]
union Arch {
    x86_64: ArchX86,
    aarch64: ArchAarch64,
    _bindgen_union_align: [u64; 8usize],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct ArchX86 {
    acpi_ptr: u64,
    smbi_ptr: u64,
    efi_ptr: u64,
    mp_ptr: u64,
    unused0: u64,
    unused1: u64,
    unused2: u64,
    unused3: u64,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct ArchAarch64 {
    acpi_ptr: u64,
    mmio_ptr: u64,
    efi_ptr: u64,
    unused0: u64,
    unused1: u64,
    unused2: u64,
    unused3: u64,
    unused4: u64,
}

#[no_mangle]
extern "C" fn _start() -> ! {
    *crate::BOOTLOADER.lock() = "BOOTBOOT";
    crate::kernel_main()
}
