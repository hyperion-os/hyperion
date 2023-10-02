#![no_std]
#![feature(new_uninit, maybe_uninit_slice, maybe_uninit_write_slice)]

//

extern crate alloc;

use alloc::boxed::Box;
use core::mem::MaybeUninit;

use elf::{
    abi::{PF_R, PF_W, PF_X, PT_LOAD},
    endian::AnyEndian,
    segment::ProgramHeader,
    ElfBytes,
};
use hyperion_arch::{syscall, vmm::PageMap};
use hyperion_mem::{from_higher_half, pmm, vmm::PageMapImpl};
use x86_64::{structures::paging::PageTableFlags, PhysAddr, VirtAddr};

//

pub struct Loader<'a> {
    parser: ElfBytes<'a, AnyEndian>,
    page_map: PageMap,
}

//

impl<'a> Loader<'a> {
    pub fn new(elf_bytes: &'a [u8]) -> Self {
        Self {
            parser: ElfBytes::minimal_parse(elf_bytes).expect("TODO: error handling"),
            page_map: PageMap::current(),
        }
    }

    pub fn load(&self) {
        // TODO: at least some safety with malicious ELFs

        let segments = self.parser.segments().expect("TODO:");

        // TODO: max segments
        for segment in segments.iter().filter(|segment| segment.p_type == PT_LOAD) {
            self.load_segment(segment);
        }

        // TODO: reloactions
    }

    pub fn load_segment(&self, segment: ProgramHeader) {
        /* hyperion_log::debug!("Loading segment {segment:#?}"); */

        let align = segment.p_align;
        let v_addr = VirtAddr::new(segment.p_vaddr)
            .align_down(align)
            .align_down(0x1000u64);
        let align_down_offs = segment.p_vaddr - v_addr.as_u64();
        let v_end = (v_addr + segment.p_memsz + align_down_offs).align_up(0x1000u64);
        let v_range = v_addr..v_end;
        let v_size = v_end - v_addr;

        if v_end > VirtAddr::new(0x400000000000) {
            hyperion_log::error!("ELF segments cannot be mapped to higher half");
            panic!("TODO:")
        }

        // TODO: max v_size

        let segment_data = self.parser.segment_data(&segment).expect("TODO:");

        let segment_alloc: &mut [MaybeUninit<u8>] =
            Box::leak(Box::new_uninit_slice(v_size as usize));

        let (segment_alloc_align_pad, segment_alloc_virtual) =
            segment_alloc.split_at_mut(align_down_offs as usize);
        let (segment_alloc_data, segment_alloc_zeros) =
            segment_alloc_virtual.split_at_mut(segment_alloc_virtual.len().min(segment_data.len()));

        segment_alloc_align_pad.fill(MaybeUninit::zeroed());
        MaybeUninit::write_slice(segment_alloc_data, segment_data);
        segment_alloc_zeros.fill(MaybeUninit::zeroed());

        // SAFETY: segment_alloc was filled with data and zeros
        let segment_alloc = unsafe { MaybeUninit::slice_assume_init_mut(segment_alloc) };

        let segment_alloc_phys =
            from_higher_half(VirtAddr::new(segment_alloc.as_ptr() as usize as u64));

        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if segment.p_flags & PF_X == 0 {
            flags.insert(PageTableFlags::NO_EXECUTE);
        }
        if segment.p_flags & PF_W != 0 {
            flags.insert(PageTableFlags::WRITABLE);
        }
        if segment.p_flags & PF_R != 0 {
            // READ is always enabled
            // TODO: read-only
        }

        /* hyperion_log::debug!(
                    "Mapping segment [ 0x{v_addr:016x}..0x{v_end:016x} -> 0x{segment_alloc_phys:016x} ] ({:03b} = {flags:?}) (0x{:016x})", segment.p_flags,
        segment.p_vaddr
                ); */
        self.page_map.map(v_range, segment_alloc_phys, flags);
    }

    pub fn debug(&self) {
        let common = self.parser.find_common_data().unwrap();
        let (dyn_symtab, dyn_strtab) = (common.dynsyms.unwrap(), common.dynsyms_strs.unwrap());
        let (symtab, strtab) = (common.symtab.unwrap(), common.symtab_strs.unwrap());

        let dyn_symbols = dyn_symtab
            .iter()
            .filter_map(|sym| dyn_strtab.get(sym.st_name as _).ok());
        let symbols = symtab
            .iter()
            .filter_map(|sym| strtab.get(sym.st_name as _).ok());

        hyperion_log::debug!("Symbols:");
        for symbol in dyn_symbols.chain(symbols) {
            hyperion_log::debug!(" - {symbol}");
        }
    }

    // TODO: impl args
    pub fn enter_userland(&self, _args: &[&str]) -> Option<()> {
        self.page_map.activate();

        // TODO: this is HIGHLY unsafe atm.

        let entrypoint = self.parser.ehdr.e_entry;

        if entrypoint == 0 {
            hyperion_log::error!("No entrypoint");
            return None;
        }

        let user_stack = pmm::PFA.alloc(1);
        let stack_top = VirtAddr::new(0x400000000000); // VirtAddr::new(hyperion_boot::hhdm_offset());

        // hyperion_log::debug!("stack_top = 0x{stack_top:016x}");

        /* self.page_map
        .unmap(VirtAddr::new_truncate(0x0000)..VirtAddr::new_truncate(0x1000)); */
        self.page_map.map(
            stack_top - 0x1000u64..stack_top,
            user_stack.physical_addr(),
            PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE,
        );
        self.page_map.map(
            stack_top - 0x2000u64..stack_top - 0x1000u64,
            user_stack.physical_addr(),
            PageTableFlags::empty(), // guard page
        );

        /* hyperion_log::debug!(
            "Entering userland at 0x{entrypoint:016x} with stack 0x{stack_top:016x}"
        ); */

        unsafe { syscall::userland(VirtAddr::new(entrypoint), stack_top) };
    }
}
