use bitflags::bitflags;
use elf::{endian::AnyEndian, string_table::StringTable};

//

#[derive(Debug)]
pub struct SectionHeader<'a> {
    pub name: &'a str,
    pub ty: SectionHeaderType,
    pub flags: SectionHeaderFlags,
    // pub virt_addr: u64,
    // pub bytes: &'a [u8],
    // pub size: u64,

    // /// Defined by section type
    // pub link: u32,
    // /// Defined by section type
    // pub info: u32,
    // /// address alignment
    // pub align: u64,
    // /// size of an entry if section data is an array of entries
    // pub entsize: u64,
}

impl<'a> SectionHeader<'a> {
    pub fn parse(
        parser: &elf::ElfBytes<'a, AnyEndian>,
        sh: elf::section::SectionHeader,
        strtab: &StringTable<'a>,
    ) -> Option<Self> {
        let name = strtab.get(sh.sh_name as usize).ok()?;
        let flags = SectionHeaderFlags::from_bits(sh.sh_flags)?;

        let (_bytes, comp) = parser.section_data(&sh).ok()?;
        assert_eq!(comp, None);

        Some(Self {
            name,
            ty: SectionHeaderType(sh.sh_type),
            flags,
            // virt_addr: sh.sh_addr,
            // bytes,
            // size: sh.sh_size,

            // link: sh.sh_link,
            // info: sh.sh_info,
            // align: sh.sh_addralign,
            // entsize: sh.sh_entsize,
        })
    }
}

//

#[derive(Debug, PartialEq, Eq)]
pub struct SectionHeaderType(u32);

#[allow(dead_code)]
impl SectionHeaderType {
    pub const NULL: Self = Self(elf::abi::SHT_NULL);
    pub const PROGBITS: Self = Self(elf::abi::SHT_PROGBITS);
    pub const SYMTAB: Self = Self(elf::abi::SHT_SYMTAB);
    pub const STRTAB: Self = Self(elf::abi::SHT_STRTAB);
    pub const RELA: Self = Self(elf::abi::SHT_RELA);
    pub const HASH: Self = Self(elf::abi::SHT_HASH);
    pub const DYNAMIC: Self = Self(elf::abi::SHT_DYNAMIC);
    pub const NOTE: Self = Self(elf::abi::SHT_NOTE);
    pub const NOBITS: Self = Self(elf::abi::SHT_NOBITS);
    pub const REL: Self = Self(elf::abi::SHT_REL);
    pub const SHLIB: Self = Self(elf::abi::SHT_SHLIB);
    pub const DYNSYM: Self = Self(elf::abi::SHT_DYNSYM);
    pub const INIT_ARRAY: Self = Self(elf::abi::SHT_INIT_ARRAY);
    pub const FINI_ARRAY: Self = Self(elf::abi::SHT_FINI_ARRAY);
    pub const PREINIT_ARRAY: Self = Self(elf::abi::SHT_PREINIT_ARRAY);
    pub const GROUP: Self = Self(elf::abi::SHT_GROUP);
    pub const SYMTAB_SHNDX: Self = Self(elf::abi::SHT_SYMTAB_SHNDX);
    pub const LOOS: Self = Self(elf::abi::SHT_LOOS);
    pub const GNU_ATTRIBUTES: Self = Self(elf::abi::SHT_GNU_ATTRIBUTES);
    pub const GNU_HASH: Self = Self(elf::abi::SHT_GNU_HASH);
    pub const GNU_LIBLIST: Self = Self(elf::abi::SHT_GNU_LIBLIST);
    pub const GNU_VERDEF: Self = Self(elf::abi::SHT_GNU_VERDEF);
    pub const GNU_VERNEED: Self = Self(elf::abi::SHT_GNU_VERNEED);
    pub const GNU_VERSYM: Self = Self(elf::abi::SHT_GNU_VERSYM);
    pub const HIOS: Self = Self(elf::abi::SHT_HIOS);
    pub const LOPROC: Self = Self(elf::abi::SHT_LOPROC);
    pub const IA_64_EXT: Self = Self(elf::abi::SHT_IA_64_EXT);
    pub const IA_64_UNWIND: Self = Self(elf::abi::SHT_IA_64_UNWIND);
    pub const HIPROC: Self = Self(elf::abi::SHT_HIPROC);
    pub const LOUSER: Self = Self(elf::abi::SHT_LOUSER);
    pub const HIUSER: Self = Self(elf::abi::SHT_HIUSER);
}

//

bitflags! {
#[derive(Debug)]
pub struct SectionHeaderFlags: u64 {
    const NONE = elf::abi::SHF_NONE as u64;
    const WRITE = elf::abi::SHF_WRITE as u64;
    const ALLOC = elf::abi::SHF_ALLOC as u64;
    const EXECINSTR = elf::abi::SHF_EXECINSTR as u64;
    const MERGE = elf::abi::SHF_MERGE as u64;
    const STRINGS = elf::abi::SHF_STRINGS as u64;
    const INFO_LINK = elf::abi::SHF_INFO_LINK as u64;
    const LINK_ORDER = elf::abi::SHF_LINK_ORDER as u64;
    const OS_NONCONFORMING = elf::abi::SHF_OS_NONCONFORMING as u64;
    const GROUP = elf::abi::SHF_GROUP as u64;
    const TLS = elf::abi::SHF_TLS as u64;
    const COMPRESSED = elf::abi::SHF_COMPRESSED as u64;
    const MASKOS = elf::abi::SHF_MASKOS as u64;
    const MASKPROC = elf::abi::SHF_MASKPROC as u64;
}
}
