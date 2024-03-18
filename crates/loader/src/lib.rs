#![no_std]
#![feature(maybe_uninit_write_slice, never_type)]

//

extern crate alloc;

use alloc::sync::Arc;
use core::{mem::MaybeUninit, ptr, slice};

use elf::{
    abi::{PF_R, PF_W, PF_X, PT_LOAD, PT_TLS},
    endian::AnyEndian,
    segment::ProgramHeader,
    ElfBytes,
};
use hyperion_log::*;
use hyperion_mem::{is_higher_half, vmm::PageMapImpl};
use hyperion_scheduler::{proc::Process, task::RunnableTask};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub struct Loader<'a> {
    parser: ElfBytes<'a, AnyEndian>,
    process: Arc<Process>,
    loaded: bool,
}

//

pub struct NoEntryPoint;
pub struct InvalidElf;
pub struct OutOfVirtMem;
pub enum LoadError {
    InvalidElf(InvalidElf),
    OutOfVirtMem(OutOfVirtMem),
}

//

impl<'a> Loader<'a> {
    pub fn new(elf_bytes: &'a [u8], process: Arc<Process>) -> Result<Self, InvalidElf> {
        Ok(Self {
            parser: ElfBytes::minimal_parse(elf_bytes).map_err(|_| InvalidElf)?,
            process,
            loaded: false,
        })
    }

    pub fn load(&mut self) -> Result<(), LoadError> {
        // TODO: at least some safety with malicious ELFs

        if let Some(segments) = self.parser.segments() {
            for segment in segments.iter() {
                self.alloc_segment(segment)?;
            }

            for segment in segments.iter() {
                self.load_segment(segment);
            }

            for segment in segments.iter() {
                self.finish_segment(segment);
            }
        }

        // TODO: reloactions

        self.loaded = true;

        Ok(())
    }

    // pub fn load_tls(&self) {}

    fn alloc_segment(&self, segment: ProgramHeader) -> Result<(), LoadError> {
        if segment.p_type != PT_LOAD && segment.p_type != PT_TLS {
            return Ok(());
        }

        let align = segment.p_align;
        let v_addr = VirtAddr::new(segment.p_vaddr)
            .align_down(align)
            .align_down(0x1000u64);
        let v_end = (VirtAddr::new(segment.p_vaddr) + segment.p_memsz).align_up(0x1000u64);
        let v_size = v_end - v_addr;
        let n_pages = v_size as usize / 0x1000;
        let init_flags = PageTableFlags::WRITABLE;

        if is_higher_half(v_end.as_u64()) {
            warn!("ELF segments cannot be mapped to higher half");
            return Err(LoadError::InvalidElf(InvalidElf));
        }

        if let Err(err) = self.process.alloc_at(n_pages, v_addr, init_flags) {
            error!("could not load ELF: out of VMEM, killing process: {err:?}");
            return Err(LoadError::OutOfVirtMem(OutOfVirtMem));
        };

        Ok(())
    }

    fn load_segment(&self, segment: ProgramHeader) {
        if segment.p_type != PT_LOAD && segment.p_type != PT_TLS {
            return;
        }

        let align = segment.p_align;
        let v_addr = VirtAddr::new(segment.p_vaddr)
            .align_down(align)
            .align_down(0x1000u64);
        let align_down_offs = segment.p_vaddr - v_addr.as_u64();
        let v_end = (VirtAddr::new(segment.p_vaddr) + segment.p_memsz).align_up(0x1000u64);
        let v_size = v_end - v_addr;

        // debug!("segment phys alloc: {phys:#x} mapped to {alloc:#x}");

        let segment_data = self.parser.segment_data(&segment).expect("TODO:");
        let segment_alloc: &mut [MaybeUninit<u8>] =
            unsafe { slice::from_raw_parts_mut(v_addr.as_mut_ptr(), v_size as usize) };

        // fill segment_alloc with segment_data and pad the end with null bytes
        let (_, segment_alloc_virtual) = segment_alloc.split_at_mut(align_down_offs as usize);
        let (segment_alloc_data, _) =
            segment_alloc_virtual.split_at_mut(segment_alloc_virtual.len().min(segment_data.len()));

        // the rust compiler will convert these to u64 or even vectors
        // already zeroed: segment_alloc_align_pad
        for (byte, elf_byte) in segment_alloc_data.iter_mut().zip(segment_data) {
            unsafe { ptr::write_volatile(byte, MaybeUninit::new(*elf_byte)) };
        }
        // already zeroed: segment_alloc_zeros

        // if it is the TLS segment, save the master TLS copy location + size
        // the scheduler will create copies for each thread
        if segment.p_type == PT_TLS {
            // TODO:
            // let master_tls = (
            //     VirtAddr::new(segment.p_vaddr),
            //     // Layout::from_size_align(v_size as _, align as _).unwrap(),
            //     Layout::from_size_align(align as _, v_size as _).unwrap(),
            // );
            // let mut loaded = false;
            // self.process.master_tls.call_once(|| {
            //     loaded = true;
            //     master_tls
            // });

            // if !loaded {
            //     todo!()
            // }
        }
    }

    fn finish_segment(&self, segment: ProgramHeader) {
        if segment.p_type != PT_LOAD {
            return;
        }

        let align = segment.p_align;
        let v_addr = VirtAddr::new(segment.p_vaddr)
            .align_down(align)
            .align_down(0x1000u64);
        let v_end = (VirtAddr::new(segment.p_vaddr) + segment.p_memsz).align_up(0x1000u64);
        let flags = Self::flags(segment.p_flags);

        // println!("remap as {flags:?}");
        self.process.address_space.remap(v_addr..v_end, flags);
    }

    fn flags(p_flags: u32) -> PageTableFlags {
        let mut flags = PageTableFlags::USER_ACCESSIBLE;
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

    pub fn finish(self) -> Result<RunnableTask, NoEntryPoint> {
        let entry = self.parser.ehdr.e_entry;
        if entry == 0 {
            Err(NoEntryPoint)
        } else {
            Ok(RunnableTask::new_in(entry, 0, self.process))
        }
    }
}
