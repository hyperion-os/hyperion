#![no_std]
#![feature(maybe_uninit_write_slice, never_type)]

//

extern crate alloc;

use alloc::{string::String, vec::Vec};
use core::{
    alloc::Layout,
    mem::{self, MaybeUninit},
    ptr, slice,
};

use elf::{
    abi::{PF_R, PF_W, PF_X, PT_LOAD, PT_TLS},
    endian::AnyEndian,
    segment::ProgramHeader,
    ElfBytes,
};
use elf_wrap::*;
use hyperion_arch::syscall;
use hyperion_log::*;
use hyperion_mem::{is_higher_half, vmm::PageMapImpl};
use hyperion_scheduler::{exit, proc::Process, process, task, ExitCode};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

mod elf_wrap;

//

pub struct Loader<'a> {
    parser: ElfBytes<'a, AnyEndian>,
}

//

#[derive(Debug)]
pub struct NoEntryPoint;

//

impl<'a> Loader<'a> {
    pub fn new(elf_bytes: &'a [u8]) -> Self {
        Self {
            parser: ElfBytes::minimal_parse(elf_bytes).expect("TODO: error handling"),
        }
    }

    pub fn load(&self) {
        // TODO: at least some safety with malicious ELFs

        let (sections, sections_strtab) = self.parser.section_headers_with_strtab().unwrap();

        let sections = sections.unwrap();
        let sections_strtab = sections_strtab.unwrap();

        for section in sections
            .into_iter()
            .filter_map(|sh| SectionHeader::parse(&self.parser, sh, &sections_strtab))
        {
            if section.ty == SectionHeaderType::NOBITS
                && section.flags.contains(
                    SectionHeaderFlags::ALLOC | SectionHeaderFlags::WRITE | SectionHeaderFlags::TLS,
                )
            {
                trace!("FOUND .tbss named `{}`", section.name);
                trace!("{section:?}");
            }

            if section.ty == SectionHeaderType::PROGBITS
                && section.flags.contains(
                    SectionHeaderFlags::ALLOC | SectionHeaderFlags::WRITE | SectionHeaderFlags::TLS,
                )
            {
                trace!("FOUND .tdata named `{}`", section.name);
                trace!("{section:?}");
            }
        }

        let segments = self.parser.segments().expect("TODO:");

        let process = process();

        for segment in segments.iter() {
            self.alloc_segment(&process, segment);
        }

        for segment in segments.iter() {
            self.load_segment(&process, segment);
        }

        for segment in segments.iter() {
            self.finish_segment(&process, segment);
        }

        // TODO: reloactions
    }

    // pub fn load_tls(&self) {}

    fn alloc_segment(&self, proc: &Process, segment: ProgramHeader) {
        if segment.p_type != PT_LOAD && segment.p_type != PT_TLS {
            return;
        }

        let align = segment.p_align;
        let v_addr = VirtAddr::new(segment.p_vaddr)
            .align_down(align)
            .align_down(0x1000u64);
        let v_end = (VirtAddr::new(segment.p_vaddr) + segment.p_memsz).align_up(0x1000u64);
        let v_size = v_end - v_addr;

        if is_higher_half(v_end.as_u64()) {
            error!("ELF segments cannot be mapped to higher half");
            exit(ExitCode::CANNOT_EXECUTE);
        }

        proc.alloc_at(v_size as usize / 0x1000, v_addr, PageTableFlags::WRITABLE)
            .unwrap_or_else(|_| {
                error!("could not load ELF: out of VMEM, killing process");
                exit(ExitCode::CANNOT_EXECUTE);
            });
    }

    fn load_segment(&self, proc: &Process, segment: ProgramHeader) {
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

        if v_addr.as_u64() == 0 {
            panic!("PIE CODE LOADING IS NOT SUPPORTED");
        }

        let segment_data = self.parser.segment_data(&segment).expect("TODO:");
        let segment_alloc: &mut [MaybeUninit<u8>] =
            unsafe { slice::from_raw_parts_mut(v_addr.as_mut_ptr(), v_size as usize) };

        // fill segment_alloc with segment_data and pad the end with null bytes
        let (_, segment_alloc_virtual) = segment_alloc.split_at_mut(align_down_offs as usize);
        let (segment_alloc_data, _) =
            segment_alloc_virtual.split_at_mut(segment_alloc_virtual.len().min(segment_data.len()));

        // the rust compiler will convert these to u64 or even vectors
        // already zeroed:
        // for byte in segment_alloc_align_pad {
        //     unsafe { ptr::write_volatile(byte, MaybeUninit::zeroed()) };
        // }
        for (byte, elf_byte) in segment_alloc_data.iter_mut().zip(segment_data) {
            unsafe { ptr::write_volatile(byte, MaybeUninit::new(*elf_byte)) };
        }
        // already zeroed:
        // for byte in segment_alloc_zeros {
        //     unsafe { ptr::write_volatile(byte, MaybeUninit::zeroed()) };
        // }

        // segment_alloc_align_pad.fill(MaybeUninit::zeroed());
        // MaybeUninit::write_slice(segment_alloc_data, segment_data);
        // segment_alloc_zeros.fill(MaybeUninit::zeroed());

        // if it is the TLS segment, save the master TLS copy location + size
        // the scheduler will create copies for each thread
        if segment.p_type == PT_TLS {
            let master_tls = (
                VirtAddr::new(segment.p_vaddr),
                // Layout::from_size_align(v_size as _, align as _).unwrap(),
                Layout::from_size_align(align as _, v_size as _).unwrap(),
            );
            let mut loaded = false;
            proc.master_tls.call_once(|| {
                loaded = true;
                master_tls
            });

            if !loaded {
                todo!()
            }
        }
    }

    fn finish_segment(&self, proc: &Process, segment: ProgramHeader) {
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
        proc.address_space.page_map.remap(v_addr..v_end, flags);
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

    pub fn finish(self) -> Result<EntryPoint, NoEntryPoint> {
        let entry = self.parser.ehdr.e_entry;
        if entry == 0 {
            Err(NoEntryPoint)
        } else {
            Ok(EntryPoint { entry })
        }
    }
}

//

pub struct EntryPoint {
    entry: u64,
}

impl EntryPoint {
    pub const fn as_ptr(&self) -> *const u8 {
        self.entry as _
    }

    pub fn enter(&self, name: String, args: Vec<String>) -> ! {
        // TODO: this is HIGHLY unsafe atm.

        let entry = self.entry;
        trace!("spawning \"{name}\" with args {args:?}");

        let env_args: Vec<&str> = [name.as_str()] // TODO: actually load binaries from vfs
            .into_iter()
            .chain(args.iter().flat_map(|args| args.split(' ')))
            .collect();

        let (stack_top, argv) = Self::init_stack(&env_args);

        // now `name`, `args` and `env_args` can be freed, because they are copied into the stack
        drop(env_args);
        drop((name, args));

        task().init_tls();

        trace!("Entering userland at 0x{entry:016x} with stack 0x{stack_top:016x} and argv:{argv:#016x}");
        syscall::userland(VirtAddr::new(entry), stack_top, argv.as_u64(), 0);
    }

    pub fn init_stack(args: &[&str]) -> (VirtAddr, VirtAddr) {
        let mut stack_top = hyperion_scheduler::task().user_stack.lock().top;

        let args_len = args.iter().map(|arg| arg.len()).sum::<usize>();
        let padding = args_len.next_multiple_of(8) - args_len; // for alignment
        for _ in 0..padding {
            push(&mut stack_top, 0u8);
        }

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
}

//

/// push items to the stack
pub fn push<T: Sized>(top: &mut VirtAddr, v: T) {
    *top -= mem::size_of::<T>();
    assert!(top.is_aligned(mem::size_of::<T>() as u64));
    unsafe { top.as_mut_ptr::<T>().write_volatile(v) };
}
