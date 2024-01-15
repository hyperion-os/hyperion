#![no_std]
#![feature(maybe_uninit_write_slice)]

//

extern crate alloc;

use core::{alloc::Layout, mem::MaybeUninit, slice};

use elf::{
    abi::{PF_R, PF_W, PF_X, PT_LOAD, PT_TLS},
    endian::AnyEndian,
    segment::ProgramHeader,
    ElfBytes,
};
use hyperion_arch::{syscall, vmm::PageMap};
use hyperion_log::*;
use hyperion_mem::{is_higher_half, to_higher_half, vmm::PageMapImpl};
use hyperion_scheduler::process;
use x86_64::{align_up, structures::paging::PageTableFlags, VirtAddr};

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

        for section in self.parser.section_headers().unwrap().into_iter() {}

        let segments = self.parser.segments().expect("TODO:");

        // let mut master_tls_copy: Option<Layout> = None;
        // for header in segments.iter().filter(|h| h.p_type == PT_TLS) {
        //     let master_tls_copy = master_tls_copy.get_or_insert_default();

        //     if !header.p_align.is_power_of_two() {
        //         panic!("align should be power of 2");
        //     }

        //     let size = align_up(header.p_memsz, header.p_align);
        //     let align = header.p_align;

        //     master_tls_copy.extend(Layout::from_size_align(size, align));
        // }

        // TODO: max segments
        for segment in segments.iter() {
            self.load_segment(segment);
        }

        // TODO: reloactions
    }

    pub fn load_segment(&self, segment: ProgramHeader) {
        debug!("Loading segment {segment:#?}");
        let flags = Self::flags(segment.p_flags);

        if segment.p_type == PT_TLS {
            debug!("TLS {flags:?}");
        }

        if segment.p_type != PT_LOAD {
            return;
        }

        let align = segment.p_align;
        let v_addr = VirtAddr::new(segment.p_vaddr)
            .align_down(align)
            .align_down(0x1000u64);
        let align_down_offs = segment.p_vaddr - v_addr.as_u64();
        let v_end = (v_addr + segment.p_memsz + align_down_offs).align_up(0x1000u64);
        let v_size = v_end - v_addr;

        if is_higher_half(v_end.as_u64()) {
            error!("ELF segments cannot be mapped to higher half");
            hyperion_scheduler::exit();
        }

        let flags = Self::flags(segment.p_flags);

        // debug!("segment alloc: {v_size:#x} at {v_addr:#x}");

        let process = process();

        let phys = process
            .alloc_at(v_size as usize / 0x1000, v_addr, flags)
            .unwrap_or_else(|_| {
                error!("could not load ELF: out of VMEM, killing process");
                hyperion_scheduler::exit();
            });

        // using the HHDM address allows writing to a page that the ELF requested to be read only
        let alloc = to_higher_half(phys);

        // debug!("segment phys alloc: {phys:#x} mapped to {alloc:#x}");

        let segment_data = self.parser.segment_data(&segment).expect("TODO:");
        let segment_alloc: &mut [MaybeUninit<u8>] =
            unsafe { slice::from_raw_parts_mut(alloc.as_mut_ptr(), v_size as usize) };

        // fill segment_alloc with segment_data and pad the end with null bytes
        let (segment_alloc_align_pad, segment_alloc_virtual) =
            segment_alloc.split_at_mut(align_down_offs as usize);
        let (segment_alloc_data, segment_alloc_zeros) =
            segment_alloc_virtual.split_at_mut(segment_alloc_virtual.len().min(segment_data.len()));

        segment_alloc_align_pad.fill(MaybeUninit::zeroed());
        MaybeUninit::write_slice(segment_alloc_data, segment_data);
        segment_alloc_zeros.fill(MaybeUninit::zeroed());
    }

    fn flags(p_flags: u32) -> PageTableFlags {
        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if p_flags & PF_X == 0 {
            flags.insert(PageTableFlags::NO_EXECUTE);
        }
        if p_flags & PF_W != 0 {
            flags.insert(PageTableFlags::WRITABLE);
        }
        if p_flags & PF_R != 0 {
            // READ is always enabled
            // TODO: read-only
        }

        flags
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

        debug!("Symbols:");
        for symbol in dyn_symbols.chain(symbols) {
            debug!(" - {symbol}");
        }
    }

    pub fn init_stack(args: &[&str]) -> (VirtAddr, VirtAddr) {
        let mut stack_top = hyperion_scheduler::task().user_stack.lock().top;

        for arg in args.iter().rev() {
            for byte in arg.as_bytes().iter().rev() {
                push(&mut stack_top, *byte);
            }
        }

        for arg in args.iter().rev() {
            push(&mut stack_top, arg.as_bytes().len());
        }

        push(&mut stack_top, args.len() as u64);
        let argv = stack_top;

        stack_top = stack_top.align_down(0x10u64); // align the stack to 16

        // push a return address 0 (8-byte) because the _start function expects
        // that the stack was 16-byte aligned when jumping into it,
        // but jumping pushes the return address (8-bytes) to effectively unalign it
        //
        // we jump into user space with sysretq, which does not push anything to the stack
        // so this has to be 'emulated'
        push(&mut stack_top, 0u64);

        (stack_top, argv)
    }

    // TODO: impl args
    pub fn enter_userland(&self, args: &[&str]) -> Option<()> {
        self.page_map.activate();

        // TODO: this is HIGHLY unsafe atm.

        let entrypoint = self.parser.ehdr.e_entry;

        if entrypoint == 0 {
            error!("No entrypoint");
            return None;
        }

        let (stack_top, argv) = Self::init_stack(args);

        debug!("Entering userland at 0x{entrypoint:016x} with stack 0x{stack_top:016x}");
        trace!("cli args init with: {:#x} {:#x}", argv.as_u64(), 69);
        unsafe { syscall::userland(VirtAddr::new(entrypoint), stack_top, argv.as_u64(), 69) };
    }
}

//

/// push items to the stack
pub fn push<T: Sized>(top: &mut VirtAddr, v: T) {
    *top -= core::mem::size_of::<T>();
    unsafe { top.as_mut_ptr::<T>().write(v) };
}
