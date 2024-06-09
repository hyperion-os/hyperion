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
use mem::frame_alloc;
use riscv64_util::{PhysAddr, VirtAddr};
use util::rle::RleMemory;

//

const SIZE_4KIB: usize = 0x1000;
const SIZE_2MIB: usize = 0x1000 * 0x200;
const SIZE_1GIB: usize = 0x1000 * 0x200 * 0x200;
const SIZE_512GIB: usize = 0x1000 * 0x200 * 0x200 * 0x200;

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
    // VPN[3] of ≥256 are (will be) shared between all address spaces
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
    /// not safe lol, I am trying to figure out a good way to sync memory maps (or even not sync),
    /// probably using atomics and making the vmm lock free is the only way
    ///
    /// for now, there are no other cores and no other (scheduler) threads so it's
    /// technically kinda somewhat maybe perhaps safe™
    pub unsafe fn get_active_mut<'a>() -> &'a mut Self {
        unsafe { &mut *Self::get_active().to_hhdm().as_ptr_mut() }
    }

    /// # Safety
    /// everything has to be mapped correctly, good luck
    pub unsafe fn activate(this: PhysAddr) {
        let satp_ppn = this.as_usize() >> 12;
        let satp_mode = 9 << 60; // 8=Sv39 , 9=Sv48 , 10=Sv57 , 11=Sv64
        let satp = satp_mode | satp_ppn;

        unsafe { asm!("csrw satp, {satp}", satp = in(reg) satp) };
    }

    pub fn map_data(&mut self, mut to: Range<VirtAddr>, flags: PageFlags, mut data: &[u8]) {
        let mut padding = to.start.offset();
        to.start = to.start.align_down();
        to.end = to.end.align_up();

        let n_4k_pages = (padding + data.len())
            .max(to.end.as_usize() - to.start.as_usize())
            .div_ceil(1 << 12);

        for i in 0..n_4k_pages {
            let entry = self.create_entry(to.start + i * 0x1000, Depth::Lvl3);

            if !entry.flags().contains(PageFlags::VALID) {
                Self::create_table_for_entry(entry);
                entry.set_flags(PageFlags::VALID | flags);
            }

            let phys_page = entry.addr();
            let phys_page: &mut [MaybeUninit<u8>; 0x1000] =
                unsafe { &mut *phys_page.to_hhdm().as_ptr_mut() };

            let copied;
            (copied, data) = data.split_at(data.len().min(0x1000 - padding));

            let mapped_copy_destination = &mut phys_page[padding..padding + copied.len()];
            MaybeUninit::<u8>::copy_from_slice(mapped_copy_destination, copied);

            padding = 0;
        }
    }

    pub fn map_data_loader(
        &mut self,
        memory: &mut RleMemory,
        mut to: Range<VirtAddr>,
        flags: PageFlags,
        mut data: &[u8],
    ) {
        let mut padding = to.start.offset();
        to.start = to.start.align_down();
        to.end = to.end.align_up();

        let n_4k_pages = (padding + data.len())
            .max(to.end.as_usize() - to.start.as_usize())
            .div_ceil(1 << 12);

        for i in 0..n_4k_pages {
            let entry = self.create_entry_loader(memory, to.start + i * 0x1000, Depth::Lvl3);

            if !entry.flags().contains(PageFlags::VALID) {
                Self::create_table_for_entry_loader(memory, entry);
                entry.set_flags(PageFlags::VALID | flags);
            }

            let phys_page = entry.addr();
            let phys_page: &mut [MaybeUninit<u8>; 0x1000] =
                unsafe { &mut *phys_page.as_phys_ptr_mut() };

            let copied;
            (copied, data) = data.split_at(data.len().min(0x1000 - padding));

            let mapped_copy_destination = &mut phys_page[padding..padding + copied.len()];
            MaybeUninit::<u8>::copy_from_slice(mapped_copy_destination, copied);

            padding = 0;
        }
    }

    pub fn map_identity(&mut self, to: Range<VirtAddr>, flags: PageFlags) {
        let from = PhysAddr::new_truncate(to.start.as_usize());
        self.map_offset(to, flags, from)
    }

    pub fn map_identity_loader(
        &mut self,
        memory: &mut RleMemory,
        to: Range<VirtAddr>,
        flags: PageFlags,
    ) {
        let from = PhysAddr::new_truncate(to.start.as_usize());
        self.map_offset_loader(memory, to, flags, from)
    }

    pub fn map_offset(&mut self, mut to: Range<VirtAddr>, flags: PageFlags, mut from: PhysAddr) {
        to.start = to.start.align_down();
        to.end = to.end.align_up();
        from = from.align_down();

        while to.start < to.end {
            // map the biggest possible page type that fits, and is aligned
            // these are 4KiB (the last layer), 2MiB (the 2nd last layer),
            // 1GiB (the 2nd layer) or 512GiB (the 1st layer)

            // FIXME: alignment can be checked just once before the loop

            let (size, depth) = Self::select_page_size(to.clone(), from);
            log::println!("mapping {size} ({depth:?}) {}->{}", to.start, from);

            let entry = self.create_entry(to.start, depth);
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

    pub fn map_offset_loader(
        &mut self,
        memory: &mut RleMemory,
        mut to: Range<VirtAddr>,
        flags: PageFlags,
        mut from: PhysAddr,
    ) {
        to.start = to.start.align_down();
        to.end = to.end.align_up();
        from = from.align_down();

        while to.start < to.end {
            // map the biggest possible page type that fits, and is aligned
            // these are 4KiB (the last layer), 2MiB (the 2nd last layer),
            // 1GiB (the 2nd layer) or 512GiB (the 1st layer)

            // FIXME: alignment can be checked just once before the loop

            let (size, depth) = Self::select_page_size(to.clone(), from);
            log::println!("mapping {size} ({depth:?}) {}->{}", to.start, from);

            let entry = self.create_entry_loader(memory, to.start, depth);
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

    fn select_page_size(to: Range<VirtAddr>, from: PhysAddr) -> (usize, Depth) {
        if to.end.abs_diff(to.start) >= SIZE_512GIB
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
        }
    }

    pub fn walk(&self, addr: VirtAddr) -> (Option<PhysAddr>, PageFlags) {
        let mut table = self;

        for idx in addr.table_indices() {
            let entry = table.entries[idx];

            if !entry.flags().contains(PageFlags::VALID) {
                return (None, PageFlags::empty());
            }

            if entry.flags().is_leaf() {
                return (Some(entry.addr()), entry.flags());
            }

            table = unsafe { &mut *entry.addr().to_hhdm().as_ptr_mut() };
        }

        unreachable!()
    }

    pub fn walk_loader(&self, addr: VirtAddr) -> (Option<PhysAddr>, PageFlags) {
        let mut table = self;

        for idx in addr.table_indices() {
            let entry = table.entries[idx];

            if !entry.flags().contains(PageFlags::VALID) {
                return (None, PageFlags::empty());
            }

            if entry.flags().is_leaf() {
                return (Some(entry.addr()), entry.flags());
            }

            table = unsafe { &mut *entry.addr().as_phys_ptr_mut() };
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

    fn create_entry<'a>(&'a mut self, at: VirtAddr, depth: Depth) -> &'a mut PageTableEntry {
        let mut table = self;

        for (i, idx) in at.table_indices().into_iter().enumerate() {
            let entry = &mut table.entries[idx];

            if i == depth as usize {
                return entry;
            }

            if !entry.flags().contains(PageFlags::VALID) {
                // initialize the next lvl table
                table = Self::create_table_for_entry(entry);
            } else {
                // the next lvl table is already initialized, or so it seems
                table = unsafe { &mut *entry.addr().to_hhdm().as_ptr_mut() };
            }
        }

        unreachable!()
    }

    fn create_entry_loader<'a>(
        &'a mut self,
        memory: &mut RleMemory,
        at: VirtAddr,
        depth: Depth,
    ) -> &'a mut PageTableEntry {
        let mut table = self;

        for (i, idx) in at.table_indices().into_iter().enumerate() {
            let entry = &mut table.entries[idx];

            if i == depth as usize {
                return entry;
            }

            if !entry.flags().contains(PageFlags::VALID) {
                // initialize the next lvl table
                table = Self::create_table_for_entry_loader(memory, entry);
            } else {
                // the next lvl table is already initialized, or so it seems
                table = unsafe { &mut *entry.addr().as_phys_ptr_mut() };
            }
        }

        unreachable!()
    }

    // fn walk_with<'a>(
    //     &'a mut self,
    //     at: VirtAddr,
    //     depth: Depth,
    //     mut f: impl FnMut(&'a mut PageTableEntry) -> Option<&'a mut Self>,
    // ) -> &mut PageTableEntry {
    //     let mut table = self;

    //     for (i, idx) in at.table_indices().into_iter().enumerate() {
    //         let entry = &mut table.entries[idx];

    //         if i == depth as usize {
    //             return entry;
    //         }

    //         let Some(_table) = f(entry) else {
    //             return entry;
    //         };

    //         table = _table;
    //     }

    //     unreachable!()
    // }

    fn create_table_for_entry<'a>(entry: &'a mut PageTableEntry) -> &'a mut PageTable {
        entry.set_flags(PageFlags::VALID);
        entry.set_addr(frame_alloc::alloc().addr());
        unsafe { Self::init_entry_as_table(entry.addr().to_hhdm().as_ptr_mut()) }
    }

    fn create_table_for_entry_loader<'a>(
        memory: &mut RleMemory,
        entry: &'a mut PageTableEntry,
    ) -> &'a mut PageTable {
        entry.set_flags(PageFlags::VALID);
        entry.set_addr(PhysAddr::new(memory.alloc()));
        unsafe { Self::init_entry_as_table(entry.addr().as_phys_ptr_mut()) }
    }

    /// # Safety
    /// assumes that the entry already has an address
    unsafe fn init_entry_as_table<'a>(entry: *mut MaybeUninit<PageTable>) -> &'a mut PageTable {
        MaybeUninit::write(unsafe { &mut *entry }, PageTable::EMPTY)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
