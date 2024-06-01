#![no_std]
#![feature(generic_nonzero)]

use core::{ffi::CStr, fmt, iter, mem, num::NonZero, ptr, str};

use log::println;
use util::rle::{Region, RleMemory};

//

// https://github.com/devicetree-org/devicetree-specification/releases/tag/v0.4
#[derive(Debug)]
#[repr(C)]
pub struct FdtHeader {
    pub magic: u32,
    pub totalsize: u32,
    pub off_dt_struct: u32,
    pub off_dt_strings: u32,
    pub off_mem_rsvmap: u32,
    pub version: u32,
    pub last_comp_version: u32,
    pub boot_cpuid_phys: u32,
    pub size_dt_strings: u32,
    pub size_dt_struct: u32,
}

impl FdtHeader {
    /// SAFETY:
    /// a1 must point to a readable and
    /// correctly aligned flattened device tree blob
    unsafe fn read(dtb_addr: *const Self) -> Self {
        let mut tree = unsafe { dtb_addr.read_unaligned() };
        // convert big-endian data into native endian
        tree.magic = tree.magic.swap_bytes();
        tree.totalsize = tree.totalsize.swap_bytes();
        tree.off_dt_struct = tree.off_dt_struct.swap_bytes();
        tree.off_dt_strings = tree.off_dt_strings.swap_bytes();
        tree.off_mem_rsvmap = tree.off_mem_rsvmap.swap_bytes();
        tree.version = tree.version.swap_bytes();
        tree.last_comp_version = tree.last_comp_version.swap_bytes();
        tree.boot_cpuid_phys = tree.boot_cpuid_phys.swap_bytes();
        tree.size_dt_strings = tree.size_dt_strings.swap_bytes();
        tree.size_dt_struct = tree.size_dt_struct.swap_bytes();
        tree
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FdtReserveEntry {
    address: u64,
    size: u64,
}

#[derive(Debug)]
pub struct Stringlist<'a>(&'a str);

impl fmt::Display for Stringlist<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut iter = self.0.split('\0');
        let Some(first) = iter.next() else {
            return Ok(());
        };
        write!(f, "{first}")?;

        for next in iter {
            write!(f, "{next}")?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum Property<'a> {
    Compatible(Stringlist<'a>),
    Model(&'a str),
    Phandle(u32),
    Status(&'a str),
    AddressCells(u32),
    SizeCells(u32),
    Reg(&'a [(u64, u64)]),
    VirtualReg(u32),
    Ranges(&'a [(u64, u64, u64)]),
    DmaRanges(&'a [(u64, u64, u64)]),
    DmaCoherent,
    DmaNonCoherent,
    Name(&'a str),
    DeviceType(&'a str),
    Other(&'a [u8]),
}

impl fmt::Display for Property<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Property::Compatible(s) => write!(f, " = {s}"),
            Property::Model(s) => write!(f, " = {s}"),
            Property::Phandle(v) => write!(f, " = {v}"),
            Property::Status(s) => write!(f, " = {s}"),
            Property::AddressCells(v) => write!(f, " = {v}"),
            Property::SizeCells(v) => write!(f, " = {v}"),
            Property::Reg(_) => todo!(),
            Property::VirtualReg(v) => write!(f, " = {v}"),
            Property::Ranges(_) => todo!(),
            Property::DmaRanges(_) => todo!(),
            Property::DmaCoherent => Ok(()),
            Property::DmaNonCoherent => Ok(()),
            Property::Name(s) => write!(f, " = {s}"),
            Property::DeviceType(s) => write!(f, " = {s}"),
            Property::Other(v) => write!(f, " = {v:?}"),
        }
    }
}

//

impl<'a> Property<'a> {
    fn from_prop(name: &str, val: &'a [u8]) -> Self {
        match name {
            "compatible" => Self::Compatible(Stringlist(str::from_utf8(val).unwrap())),
            "model" => Self::Model(str::from_utf8(val).unwrap()),
            "phandle" => Self::Phandle(u32::from_be_bytes(val.try_into().unwrap())),
            "status" => Self::Status(str::from_utf8(val).unwrap()),
            "#address-cells" => Self::AddressCells(u32::from_be_bytes(val.try_into().unwrap())),
            "#size-cells" => Self::SizeCells(u32::from_be_bytes(val.try_into().unwrap())),
            // reg
            "virtual-reg" => Self::VirtualReg(u32::from_be_bytes(val.try_into().unwrap())),
            // ranges
            // dma-ranges
            "dma-coherent" => Self::DmaCoherent,
            "dma-noncoherent" => Self::DmaNonCoherent,
            "name" => Self::Name(str::from_utf8(val).unwrap()),
            "device_type" => Self::DeviceType(str::from_utf8(val).unwrap()),
            _ => Self::Other(val),
        }
    }
}

/// FDT = Flattened Devicetree
/// DTB = Devicetree blob
#[derive(Debug)]
pub struct Fdt {
    addr: *const (),
    pub header: FdtHeader,
}

impl Fdt {
    /// # Safety
    /// a1 must point to a readable and
    /// correctly aligned flattened device tree blob
    pub unsafe fn read(a1: *const ()) -> Result<Self, &'static str> {
        let dtb_addr = a1 as *const FdtHeader;
        let header = unsafe { FdtHeader::read(dtb_addr) };

        if header.magic != 0xd00dfeed {
            return Err("a1 should be a pointer to a DTB");
        }

        Ok(Self { addr: a1, header })
    }

    pub fn iter_reserved_memory(&self) -> impl Iterator<Item = FdtReserveEntry> {
        let mut next_entry = unsafe { self.addr.byte_add(self.header.off_mem_rsvmap as usize) }
            as *const FdtReserveEntry;

        iter::from_fn(move || {
            let mut next = unsafe { next_entry.read_unaligned() };
            // convert big-endian data into native endian
            next.address = next.address.swap_bytes();
            next.size = next.size.swap_bytes();

            if next.address == 0 && next.size == 0 {
                // the first 0,0 entry ends the array
                None
            } else {
                next_entry = unsafe { next_entry.add(1) };
                Some(next)
            }
        })
    }

    // FIXME: remove the limitation of memory regions
    pub fn usable_memory(&mut self) -> RleMemory {
        let strings = unsafe { self.addr.byte_add(self.header.off_dt_strings as usize) } as _;
        let tokens = unsafe { self.addr.byte_add(self.header.off_dt_struct as usize) } as _;

        let mut tokens = unsafe { StructureParser::from_raw(strings, tokens) };

        tokens.clone().print_tree(0);

        let Some(Token::BeginNode("")) = tokens.next() else {
            panic!("invalid device tree");
        };

        let mut memory = RleMemory::new();

        tokens.clone().parse_root_memory(|region| {
            if let Some(size) = NonZero::new(region.len()) {
                memory.insert(Region {
                    addr: region as *mut u8 as usize,
                    size,
                });
            }
        });
        tokens.clone().parse_root_reserved_memory(|region| {
            if let Some(size) = NonZero::new(region.len()) {
                memory.remove(Region {
                    addr: region as *mut u8 as usize,
                    size,
                });
            }
        });

        for extra in self.iter_reserved_memory() {
            if let Some(size) = NonZero::new(extra.size as usize) {
                memory.remove(Region {
                    addr: extra.address as usize,
                    size,
                });
            }
        }

        memory
    }
}

#[derive(Clone)]
pub struct StructureParser {
    strings: *const u8,
    next_token: *const u8,
}

impl StructureParser {
    pub fn print_tree(&mut self, depth: usize) {
        loop {
            match self.next() {
                Some(Token::BeginNode(name)) => {
                    let name = if name.is_empty() && depth == 0 {
                        "/"
                    } else {
                        name
                    };

                    println!("{:depth$}{name} {{", ' ');
                    self.print_tree(depth + 4);
                    println!("{:depth$}}}", ' ');
                }
                Some(Token::Prop(name, val)) => {
                    println!("{:depth$}{name}{}", ' ', Property::from_prop(name, val));
                }
                Some(Token::EndNode) | None => return,
            }
        }
    }

    /// skip until a matching EndNode is found,
    /// assuming that the BeginNode was already found
    pub fn skip_node(&mut self) {
        let mut n = 1;
        while n != 0 {
            match self.next() {
                Some(Token::BeginNode(_)) => n += 1,
                Some(Token::EndNode) => n -= 1,
                None => panic!("invalid device tree"),
                _ => {}
            }
        }
    }

    // fn parse_root_uart(&mut self, mut uart_callback: impl FnMut(*mut [u8])) {
    //     loop {
    //         match self.next() {
    //             Some(Token::BeginNode("soc")) => self.parse_soc_uart(&mut uart_callback),
    //             Some(Token::BeginNode(_)) => {
    //                 self.skip_node();
    //             }
    //             Some(Token::Prop(..)) => {}
    //             Some(Token::EndNode) => return,
    //             None => panic!("invalid device tree"),
    //         }
    //     }
    // }

    // fn parse_soc_uart(&mut self, uart_callback: &mut impl FnMut(*mut [u8])) {
    //     let mut address_cells = 2u32;
    //     let mut size_cells = 1u32;

    //     loop {
    //         match self.next() {
    //             Some(Token::BeginNode(memory)) if memory.starts_with("memory@") => {
    //                 self.parse_memory(&mut memory_callback, address_cells, size_cells)
    //             }
    //             Some(Token::BeginNode(_)) => {
    //                 self.skip_node();
    //             }
    //             Some(Token::Prop(name, val)) => match Property::from_prop(name, val) {
    //                 Property::AddressCells(c) => address_cells = c,
    //                 Property::SizeCells(c) => size_cells = c,
    //                 _ => {}
    //             },
    //             Some(Token::EndNode) => return,
    //             None => panic!("invalid device tree"),
    //         }
    //     }
    // }

    // fn parse_soc_device_uart(
    //     &mut self,
    //     uart_callback: &mut impl FnMut(*mut [u8]),
    //     address_cells: u32,
    //     size_cells: u32,
    // ) {
    //     loop {
    //         match self.next() {
    //             Some(Token::BeginNode(_)) => {
    //                 self.skip_node();
    //             }
    //             Some(Token::Prop(name, val)) => match Property::from_prop(name, val) {
    //                 Property::AddressCells(c) => address_cells = c,
    //                 Property::SizeCells(c) => size_cells = c,
    //                 _ => {}
    //             },
    //             Some(Token::EndNode) => return,
    //             None => panic!("invalid device tree"),
    //         }
    //     }
    // }

    fn parse_root_memory(&mut self, mut memory_callback: impl FnMut(*mut [u8])) {
        let mut address_cells = 2u32;
        let mut size_cells = 1u32;

        loop {
            match self.next() {
                Some(Token::BeginNode(memory)) if memory.starts_with("memory@") => {
                    self.parse_memory(&mut memory_callback, address_cells, size_cells)
                }
                Some(Token::BeginNode(_)) => {
                    self.skip_node();
                }
                Some(Token::Prop(name, val)) => match Property::from_prop(name, val) {
                    Property::AddressCells(c) => address_cells = c,
                    Property::SizeCells(c) => size_cells = c,
                    _ => {}
                },
                Some(Token::EndNode) => return,
                None => panic!("invalid device tree"),
            }
        }
    }

    fn parse_root_reserved_memory(&mut self, mut reserved_memory_callback: impl FnMut(*mut [u8])) {
        loop {
            match self.next() {
                Some(Token::BeginNode("reserved-memory")) => {
                    self.parse_reserved_memory(&mut reserved_memory_callback)
                }
                Some(Token::BeginNode(_)) => {
                    self.skip_node();
                }
                Some(Token::Prop(..)) => {}
                Some(Token::EndNode) => return,
                None => panic!("invalid device tree"),
            }
        }
    }

    fn parse_reserved_memory(&mut self, reserved_memory_callback: &mut impl FnMut(*mut [u8])) {
        let mut address_cells = 2u32;
        let mut size_cells = 1u32;

        loop {
            match self.next() {
                Some(Token::Prop(name, val)) => match Property::from_prop(name, val) {
                    Property::AddressCells(c) => address_cells = c,
                    Property::SizeCells(c) => size_cells = c,
                    _ => {}
                },
                Some(Token::BeginNode(_)) => self.parse_reserved_memory_entry(
                    reserved_memory_callback,
                    address_cells,
                    size_cells,
                ),
                Some(Token::EndNode) => return,
                None => panic!("invalid device tree"),
            }
        }
    }

    fn parse_reserved_memory_entry(
        &mut self,
        reserved_memory_callback: &mut impl FnMut(*mut [u8]),
        address_cells: u32,
        size_cells: u32,
    ) {
        loop {
            match self.next() {
                Some(Token::Prop(name, mut val)) => {
                    if name != "reg" {
                        continue;
                    }

                    while !val.is_empty() {
                        let (addr, size);

                        // MARK (may panic): hidden unwrap
                        // should not panic if the device tree is valid
                        (addr, val) = val.split_at(address_cells as usize * 4); // remember, a cell is 4 bytes
                        (size, val) = val.split_at(size_cells as usize * 4);

                        let addr = be_to_int(addr);
                        let size = be_to_int(size);

                        reserved_memory_callback(ptr::slice_from_raw_parts_mut(addr as _, size));
                        // println!(
                        //     "Reserved memory ({entry_name}): {:#x}..{:#x}",
                        //     addr,
                        //     addr + size
                        // );
                    }
                }
                Some(Token::EndNode) => return,
                Some(Token::BeginNode(_)) | None => panic!("invalid device tree"),
            }
        }
    }

    fn parse_memory(
        &mut self,
        memory_callback: &mut impl FnMut(*mut [u8]),
        address_cells: u32,
        size_cells: u32,
    ) {
        loop {
            match self.next() {
                Some(Token::Prop(name, mut val)) => {
                    if name != "reg" {
                        continue;
                    }

                    while !val.is_empty() {
                        let (addr, size);

                        // MARK (may panic): hidden unwrap
                        // should not panic if the device tree is valid
                        (addr, val) = val.split_at(address_cells as usize * 4); // remember, a cell is 4 bytes
                        (size, val) = val.split_at(size_cells as usize * 4);

                        let addr = be_to_int(addr);
                        let size = be_to_int(size);

                        memory_callback(ptr::slice_from_raw_parts_mut(addr as _, size));
                        // println!("Memory: {:#x}..{:#x}", addr, addr + size);
                    }
                }
                Some(Token::EndNode) => return,
                Some(Token::BeginNode(_)) | None => panic!("invalid device tree"),
            }
        }
    }

    unsafe fn from_raw(strings: *const u8, root: *const u8) -> Self {
        Self {
            strings,
            next_token: root,
        }
    }
}

impl Iterator for StructureParser {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        const FDT_BEGIN_NODE: u32 = 0x1;
        const FDT_END_NODE: u32 = 0x2;
        const FDT_PROP: u32 = 0x3;
        const FDT_NOP: u32 = 0x4;
        const FDT_END: u32 = 0x9;

        match next_token(&mut self.next_token) {
            FDT_BEGIN_NODE => {
                let name = next_cstr(&mut self.next_token)
                    .to_str()
                    .unwrap_or("<invalid-utf8>");

                Some(Token::BeginNode(name))
            }
            FDT_END_NODE => Some(Token::EndNode),
            FDT_PROP => {
                let len = next_token(&mut self.next_token);
                let nameoff = next_token(&mut self.next_token);

                let name = unsafe { CStr::from_ptr(self.strings.add(nameoff as usize).cast()) }
                    .to_str()
                    .unwrap_or("<invalid-utf8>");

                let val = ptr::slice_from_raw_parts(self.next_token, len as usize);
                let val = unsafe { &*val };
                // let val = Property::from_prop(name, val);

                self.next_token = unsafe { self.next_token.add(len as usize) };
                align(&mut self.next_token);

                Some(Token::Prop(name, val))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Token {
    BeginNode(&'static str),
    Prop(&'static str, &'static [u8]),
    EndNode,
}

fn be_to_int(be: &[u8]) -> usize {
    let mut val = 0usize;

    if be.len() > mem::size_of::<usize>() {
        todo!("address-cells/size-cells was larger than the max int");
    }

    for byte in be {
        val *= 256;
        val += *byte as usize;
    }

    val
}

fn next_token(tokens: &mut *const u8) -> u32 {
    let tok = unsafe { tokens.cast::<u32>().read() }.swap_bytes();
    *tokens = unsafe { tokens.add(4) };
    tok
}

fn next_cstr<'a>(tokens: &mut *const u8) -> &'a CStr {
    let name = unsafe { CStr::from_ptr(tokens.cast()) };
    *tokens = unsafe { tokens.add(name.count_bytes() + 1) };
    align(tokens);
    name
}

fn align(tokens: &mut *const u8) {
    *tokens = unsafe { tokens.add(tokens.align_offset(4)) };
}
