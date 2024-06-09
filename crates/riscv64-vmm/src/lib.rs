#![no_std]
#![feature(
    inline_const,
    const_option,
    const_result,
    maybe_uninit_slice,
    maybe_uninit_write_slice
)]

//! https://riscv.org/wp-content/uploads/2019/08/riscv-privileged-20190608-1.pdf

//

use bitflags::bitflags;
use core::{arch::asm, mem::MaybeUninit, ops::Range};
use riscv64_util::{PhysAddr, VirtAddr};
use util::rle::RleMemory;

//

pub trait PhysAddrAccess: Clone + Copy {
    fn phys_to_ptr<T>(&self, phys: PhysAddr) -> *mut T;
}

#[derive(Clone, Copy)]
pub struct Hhdm;

impl PhysAddrAccess for Hhdm {
    fn phys_to_ptr<T>(&self, phys: PhysAddr) -> *mut T {
        phys.to_hhdm().as_ptr_mut()
    }
}

#[derive(Clone, Copy)]
pub struct NoPaging;

impl PhysAddrAccess for NoPaging {
    fn phys_to_ptr<T>(&self, phys: PhysAddr) -> *mut T {
        phys.as_phys_ptr_mut()
    }
}

//

#[repr(C)]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl PageTable {
    pub const EMPTY: Self = Self {
        entries: [const { PageTableEntry::empty() }; 512],
    };

    pub const fn new() -> Self {
        Self::EMPTY
    }

    pub fn get_active() -> PhysAddr {
        let satp: usize;
        unsafe { asm!("csrr {satp}, satp", satp = out(reg) satp) };
        let satp_mode = satp >> 60;
        let satp_ppn = satp << 12;

        assert_eq!(satp_mode, 9);
        PhysAddr::new(satp_ppn)
    }

    /// # Safety
    /// everything has to be mapped correctly, good luck
    pub unsafe fn activate(this: PhysAddr) {
        let satp_ppn = this.as_usize() >> 12;
        let satp_mode = 9 << 60; // 8=Sv39 , 9=Sv48 , 10=Sv57 , 11=Sv64
        let satp = satp_mode | satp_ppn;

        unsafe { asm!("csrw satp, {satp}", satp = in(reg) satp) };
    }

    pub fn map(
        &mut self,
        memory: &mut RleMemory,
        to: Range<VirtAddr>,
        flags: PageFlags,
        from: &[u8],
    ) {
        self._map(memory, to, flags, from, Hhdm)
    }

    pub fn map_without_paging(
        &mut self,
        memory: &mut RleMemory,
        to: Range<VirtAddr>,
        flags: PageFlags,
        from: &[u8],
    ) {
        self._map(memory, to, flags, from, NoPaging)
    }

    fn _map(
        &mut self,
        memory: &mut RleMemory,
        mut to: Range<VirtAddr>,
        flags: PageFlags,
        mut from: &[u8],
        ty: impl PhysAddrAccess,
    ) {
        let mut padding = to.start.offset();
        to.start = to.start.align_down();
        to.end = to.end.align_up();

        let n_4k_pages = (padding + from.len())
            .max(to.end.as_usize() - to.start.as_usize())
            .div_ceil(1 << 12);

        for i in 0..n_4k_pages {
            let entry = self.create_entry(memory, to.start + i * 0x1000, Depth::Lvl3, ty);

            if !entry.flags().contains(PageFlags::VALID) {
                Self::create_table_for_entry(memory, entry, ty);
                entry.set_flags(PageFlags::VALID | flags);
            }

            let phys_page = entry.addr();
            let phys_page: &mut [MaybeUninit<u8>; 0x1000] =
                unsafe { &mut *ty.phys_to_ptr(phys_page) };

            let copied;
            (copied, from) = from.split_at(from.len().min(0x1000 - padding));

            let mapped_copy_destination = &mut phys_page[padding..padding + copied.len()];
            MaybeUninit::<u8>::copy_from_slice(mapped_copy_destination, copied);

            padding = 0;
        }
    }

    pub fn map_identity(&mut self, memory: &mut RleMemory, to: Range<VirtAddr>, flags: PageFlags) {
        self._map_identity(memory, to, flags, Hhdm)
    }

    pub fn map_identity_without_paging(
        &mut self,
        memory: &mut RleMemory,
        to: Range<VirtAddr>,
        flags: PageFlags,
    ) {
        self._map_identity(memory, to, flags, NoPaging)
    }

    fn _map_identity(
        &mut self,
        memory: &mut RleMemory,
        to: Range<VirtAddr>,
        flags: PageFlags,
        ty: impl PhysAddrAccess,
    ) {
        let from = PhysAddr::new_truncate(to.start.as_usize());
        self._map_offset(memory, to, flags, from, ty);
    }

    pub fn map_offset(
        &mut self,
        memory: &mut RleMemory,
        to: Range<VirtAddr>,
        flags: PageFlags,
        from: PhysAddr,
    ) {
        self._map_offset(memory, to, flags, from, Hhdm)
    }

    pub fn map_offset_without_paging(
        &mut self,
        memory: &mut RleMemory,
        to: Range<VirtAddr>,
        flags: PageFlags,
        from: PhysAddr,
    ) {
        self._map_offset(memory, to, flags, from, NoPaging)
    }

    fn _map_offset(
        &mut self,
        memory: &mut RleMemory,
        mut to: Range<VirtAddr>,
        flags: PageFlags,
        mut from: PhysAddr,
        ty: impl PhysAddrAccess,
    ) {
        to.start = to.start.align_down();
        to.end = to.end.align_up();
        from = from.align_down();

        // println!(
        //     "mapping {size} ({depth:?}) {:#x}->{:#x}",
        //     to.start.0, from.0
        // );

        while to.start < to.end {
            // map the biggest possible page type that fits, and is aligned
            // these are 4KiB (the last layer), 2MiB (the 2nd last layer),
            // 1GiB (the 2nd layer) or 512GiB (the 1st layer)

            const SIZE_4KIB: usize = 0x1000;
            const SIZE_2MIB: usize = 0x1000 * 0x200;
            const SIZE_1GIB: usize = 0x1000 * 0x200 * 0x200;
            const SIZE_512GIB: usize = 0x1000 * 0x200 * 0x200 * 0x200;

            // FIXME: alignment can be checked just once before the loop

            let (size, depth) = if to.end.abs_diff(to.start) >= SIZE_512GIB
                && to.start.is_aligned_to(SIZE_512GIB)
                && from.is_aligned_to(SIZE_512GIB)
            {
                (SIZE_512GIB, Depth::Lvl0)
            } else if to.end.abs_diff(to.start) >= SIZE_1GIB
                && to.start.is_aligned_to(SIZE_1GIB)
                && from.is_aligned_to(SIZE_1GIB)
            {
                (SIZE_1GIB, Depth::Lvl1)
            } else if to.end.abs_diff(to.start) >= SIZE_2MIB
                && to.start.is_aligned_to(SIZE_2MIB)
                && from.is_aligned_to(SIZE_2MIB)
            {
                assert!(to.start.table_indices()[3] == 0);
                (SIZE_2MIB, Depth::Lvl2)
            } else {
                (SIZE_4KIB, Depth::Lvl3)
            };

            log::println!("mapping {size} ({depth:?}) {}->{}", to.start, from);
            let entry = self.create_entry(memory, to.start, depth, ty);

            if entry.flags().contains(PageFlags::VALID) {
                panic!("already mapped");
            }

            entry.set_flags(PageFlags::VALID | flags);
            entry.set_addr(from);
            core::hint::black_box(entry);

            to.start += size;
            from += size;
        }
    }

    pub fn walk(&self, addr: VirtAddr) -> (Option<PhysAddr>, PageFlags) {
        self._walk(addr, Hhdm)
    }

    pub fn walk_without_paging(&self, addr: VirtAddr) -> (Option<PhysAddr>, PageFlags) {
        self._walk(addr, NoPaging)
    }

    fn _walk(&self, addr: VirtAddr, ty: impl PhysAddrAccess) -> (Option<PhysAddr>, PageFlags) {
        let mut table = self;

        for idx in addr.table_indices() {
            let entry = table.entries[idx];

            if !entry.flags().contains(PageFlags::VALID) {
                return (None, PageFlags::empty());
            }

            if entry.flags().is_leaf() {
                return (Some(entry.addr()), entry.flags());
            }

            table = unsafe { &mut *ty.phys_to_ptr(entry.addr()) };
        }

        unreachable!()
    }

    pub fn alloc_page_table(
        memory: &mut RleMemory,
        ty: impl PhysAddrAccess,
    ) -> &'static mut PageTable {
        let page = PhysAddr::new(memory.alloc());
        unsafe { &mut *ty.phys_to_ptr::<MaybeUninit<PageTable>>(page) }.write(PageTable::EMPTY)
    }

    fn create_entry<'a>(
        &'a mut self,
        memory: &mut RleMemory,
        at: VirtAddr,
        depth: Depth,
        ty: impl PhysAddrAccess,
    ) -> &'a mut PageTableEntry {
        let mut table = self;

        for (i, idx) in at.table_indices().into_iter().enumerate() {
            let entry = &mut table.entries[idx];

            if i == depth as usize {
                return entry;
            }

            if !entry.flags().contains(PageFlags::VALID) {
                // initialize the next lvl table
                table = Self::create_table_for_entry(memory, entry, ty);
            } else {
                // the next lvl table is already initialized, or so it seems
                table = unsafe { &mut *ty.phys_to_ptr(entry.addr()) };
            }
        }

        unreachable!()
    }

    fn create_table_for_entry<'a>(
        memory: &mut RleMemory,
        entry: &'a mut PageTableEntry,
        ty: impl PhysAddrAccess,
    ) -> &'a mut PageTable {
        entry.set_flags(PageFlags::VALID);
        entry.set_addr(PhysAddr::new(memory.alloc()));

        let next_table_ptr = ty.phys_to_ptr(entry.addr());
        core::hint::black_box(MaybeUninit::write(
            unsafe { &mut *next_table_ptr },
            PageTable::EMPTY,
        ))
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

//

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum Depth {
    Lvl0 = 0,
    Lvl1 = 1,
    Lvl2 = 2,
    Lvl3 = 3,
}

//

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PageTableEntry(usize);

impl PageTableEntry {
    pub const EMPTY: Self = Self(0);

    pub const fn empty() -> Self {
        Self::EMPTY
    }

    pub const fn new(a: PhysAddr, f: PageFlags) -> Self {
        Self(((a.as_usize() >> 12) << 10) | f.bits() as usize)
    }

    pub fn set_addr(&mut self, a: PhysAddr) {
        *self = Self::new(a, self.flags());
    }

    pub fn set_flags(&mut self, f: PageFlags) {
        *self = Self::new(self.addr(), f);
    }

    pub fn addr(self) -> PhysAddr {
        PhysAddr::new_truncate(((self.0 >> 10) & ((1 << 44) - 1)) << 12)
    }

    pub fn flags(self) -> PageFlags {
        PageFlags::from_bits_truncate((self.0 & ((1 << 10) - 1)) as u16)
    }
}

//

bitflags! {
#[derive(Debug, Clone, Copy)]
pub struct PageFlags: u16 {
    const VALID = 1 << 0;
    const R = 1 << 1;
    const W = 1 << 2;
    const X = 1 << 3;
    const USER = 1 << 4;
    const GLOBAL = 1 << 5;
    const ACCESSED = 1 << 6;
    const DIRTY = 1 << 7;

    const RW = Self::R.bits() | Self::W.bits();
    const RX = Self::R.bits() | Self::X.bits();
    const WX = Self::W.bits() | Self::X.bits();
    const RWX = Self::R.bits() | Self::W.bits() | Self::X.bits();
}
}

impl PageFlags {
    pub const fn is_branch(self) -> bool {
        self.intersection(Self::RWX).is_empty()
    }

    pub const fn is_leaf(self) -> bool {
        !self.is_branch()
    }
}
