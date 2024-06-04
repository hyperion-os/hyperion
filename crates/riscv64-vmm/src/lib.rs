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
use core::{
    arch::asm,
    fmt,
    mem::MaybeUninit,
    ops::{Add, AddAssign, Range},
};
use util::rle::RleMemory;

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

    /// # Safety
    /// everything has to be mapped correctly, good luck
    pub unsafe fn activate(this: *mut Self) {
        let satp_ppn = this as usize >> 12;
        let satp_mode = 9 << 60; // 8=Sv39 , 9=Sv48 , 10=Sv57 , 11=Sv64
        let satp = satp_mode | satp_ppn;

        unsafe { asm!("csrw satp, {satp}", satp = in(reg) satp) };
    }

    pub fn map(
        &mut self,
        memory: &mut RleMemory,
        mut to: Range<VirtAddr>,
        flags: PageFlags,
        mut from: &[u8],
    ) {
        let mut padding = to.start.offset();
        to.start = to.start.align_down();
        to.end = to.end.align_up();

        let n_4k_pages = (padding + from.len())
            .max(to.end.0 - to.start.0)
            .div_ceil(1 << 12);

        for i in 0..n_4k_pages {
            let entry = self.create_entry(memory, VirtAddr(to.start.0 + i * 0x1000), Depth::Lvl3);

            if !entry.flags().contains(PageFlags::VALID) {
                Self::create_table_for_entry(memory, entry);
                entry.set_flags(PageFlags::VALID | flags);
            }

            let phys_page = entry.addr();

            // // zero the page
            // let phys_page_ptr = phys_page.0 as *mut MaybeUninit<PageTable>;
            // MaybeUninit::write(unsafe { &mut *phys_page_ptr }, PageTable::EMPTY);

            let phys_page = unsafe { &mut *(phys_page.0 as *mut [MaybeUninit<u8>; 0x1000]) };

            let copied;
            (copied, from) = from.split_at(from.len().min(0x1000 - padding));

            let mapped_copy_destination = &mut phys_page[padding..padding + copied.len()];
            MaybeUninit::<u8>::copy_from_slice(mapped_copy_destination, copied);

            padding = 0;
        }
    }

    pub fn map_identity(&mut self, memory: &mut RleMemory, to: Range<VirtAddr>, flags: PageFlags) {
        let from = PhysAddr::new_truncate(to.start.0);
        self.map_offset(memory, to, flags, from);
    }

    pub fn map_offset(
        &mut self,
        memory: &mut RleMemory,
        mut to: Range<VirtAddr>,
        flags: PageFlags,
        mut from: PhysAddr,
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

            let (size, depth) = if to.end.0.abs_diff(to.start.0) >= SIZE_512GIB
                && is_aligned(to.start.0, SIZE_512GIB)
                && is_aligned(from.0, SIZE_512GIB)
            {
                (SIZE_512GIB, Depth::Lvl0)
            } else if to.end.0.abs_diff(to.start.0) >= SIZE_1GIB
                && is_aligned(to.start.0, SIZE_1GIB)
                && is_aligned(from.0, SIZE_1GIB)
            {
                (SIZE_1GIB, Depth::Lvl1)
            } else if to.end.0.abs_diff(to.start.0) >= SIZE_2MIB
                && is_aligned(to.start.0, SIZE_2MIB)
                && is_aligned(from.0, SIZE_2MIB)
            {
                assert!(to.start.table_indices()[3] == 0);
                (SIZE_2MIB, Depth::Lvl2)
            } else {
                (SIZE_4KIB, Depth::Lvl3)
            };

            // println!(
            //     "mapping {size} ({depth:?}) {:#x}->{:#x}",
            //     to.start.0, from.0
            // );
            let entry = self.create_entry(memory, to.start, depth);

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

    pub fn create_entry<'a>(
        &'a mut self,
        memory: &mut RleMemory,
        at: VirtAddr,
        depth: Depth,
        // flags: PageFlags,
    ) -> &'a mut PageTableEntry {
        let mut table = self;

        for (i, idx) in at.table_indices().into_iter().enumerate() {
            let entry = &mut table.entries[idx];

            if i == depth as usize {
                return entry;
            }

            if !entry.flags().contains(PageFlags::VALID) {
                // initialize the next lvl table
                table = Self::create_table_for_entry(memory, entry);
            } else {
                // the next lvl table is already initialized, or so it seems
                let next_table_ptr = entry.addr().0 as *mut PageTable;
                table = unsafe { &mut *next_table_ptr };
            }
        }

        unreachable!()
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

            let next_table_ptr = entry.addr().0 as *mut PageTable;
            table = unsafe { &mut *next_table_ptr };
        }

        unreachable!()
    }

    pub fn create_table_for_entry<'a>(
        memory: &mut RleMemory,
        entry: &'a mut PageTableEntry,
    ) -> &'a mut PageTable {
        entry.set_flags(PageFlags::VALID);
        entry.set_addr(PhysAddr(memory.alloc()));

        let next_table_ptr = entry.addr().0 as *mut MaybeUninit<PageTable>;
        core::hint::black_box(MaybeUninit::write(
            unsafe { &mut *next_table_ptr },
            PageTable::EMPTY,
        ))
    }

    pub fn alloc_page_table(memory: &mut RleMemory) -> &'static mut PageTable {
        let page = memory.alloc();
        let page_ptr = page as *mut MaybeUninit<PageTable>;
        unsafe { &mut *page_ptr }.write(PageTable::EMPTY)
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
        Self(((a.0 >> 12) << 10) | f.bits() as usize)
    }

    pub fn set_addr(&mut self, a: PhysAddr) {
        *self = Self::new(a, self.flags());
    }

    pub fn set_flags(&mut self, f: PageFlags) {
        *self = Self::new(self.addr(), f);
    }

    pub fn addr(self) -> PhysAddr {
        PhysAddr(((self.0 >> 10) & ((1 << 44) - 1)) << 12)
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

//

#[derive(Debug, Clone, Copy)]
pub struct PhysAddr(usize);

impl PhysAddr {
    pub const fn new(addr: usize) -> Self {
        match Self::try_from(addr) {
            Ok(v) => v,
            Err(_) => panic!("invalid PhysAddr"),
        }
    }

    pub const fn try_from(addr: usize) -> Result<Self, InvalidAddress> {
        if Self::new_truncate(addr).0 == addr {
            Ok(Self(addr))
        } else {
            Err(InvalidAddress)
        }
    }

    pub const fn new_truncate(addr: usize) -> Self {
        Self(addr & ((1 << 52) - 1))
    }

    pub const fn from_hhdm(v: VirtAddr) -> PhysAddr {
        if VirtAddr::HHDM.0 <= v.0 && v.0 < VirtAddr::KERNEL.0 {
            PhysAddr::new(v.0 + VirtAddr::HHDM.0)
        } else {
            panic!("not a HHDM address");
        }
    }

    pub const fn to_hhdm(self) -> VirtAddr {
        VirtAddr::new(self.0 + VirtAddr::HHDM.0)
    }

    pub const fn align_up(self) -> Self {
        Self::new(align_up(self.0, 1 << 12))
    }

    pub const fn align_down(self) -> Self {
        Self::new(align_down(self.0, 1 << 12))
    }

    /// DOESN'T DO ANY ADDRESS TRANSLATIONS
    pub fn from_phys_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as _)
    }

    pub const fn null() -> Self {
        Self::new_truncate(0)
    }

    /// DOESN'T DO ANY ADDRESS TRANSLATIONS
    pub const fn as_phys_ptr<T>(self) -> *const T {
        self.0 as _
    }

    /// DOESN'T DO ANY ADDRESS TRANSLATIONS
    pub const fn as_phys_ptr_mut<T>(self) -> *mut T {
        self.0 as _
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl Add for PhysAddr {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.0.checked_add(rhs.0).unwrap())
    }
}

impl Add<usize> for PhysAddr {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self::new(self.0.checked_add(rhs).unwrap())
    }
}

impl AddAssign for PhysAddr {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.add(rhs);
    }
}

impl AddAssign<usize> for PhysAddr {
    fn add_assign(&mut self, rhs: usize) {
        *self = self.add(rhs);
    }
}

impl fmt::Display for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "p{:#x}", self.0)
    }
}

//

/// Sv48:
/// - `48..=63` : must be zero
/// - `39..=47` : level-3 index
/// - `30..=38` : level-2 index
/// - `21..=29` : level-1 index
/// - `12..=20` : level-0 index
/// - ` 0..=11` : byte offset
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr(usize);

impl VirtAddr {
    pub const OFFSET_MASK: usize = (1 << 12) - 1;
    pub const INDEX_MASK: usize = (1 << 9) - 1;

    pub const HHDM: Self = Self::new(0xFFFF800000000000);
    pub const KERNEL: Self = Self::new(0xffffffff80000000);

    pub const fn new(addr: usize) -> Self {
        match Self::try_from(addr) {
            Ok(v) => v,
            Err(_) => panic!("invalid VirtAddr"),
        }
    }

    pub const fn try_from(addr: usize) -> Result<Self, InvalidAddress> {
        if Self::new_truncate(addr).0 == addr {
            Ok(Self(addr))
        } else {
            Err(InvalidAddress)
        }
    }

    pub const fn new_truncate(addr: usize) -> Self {
        // sign extend the last valid bit
        Self(((addr << 16) as isize >> 16) as _)
    }

    // pub const fn

    pub const fn align_up(self) -> Self {
        Self::new(align_up(self.0, 1 << 12))
    }

    pub const fn align_down(self) -> Self {
        Self::new(align_down(self.0, 1 << 12))
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as _)
    }

    pub const fn null() -> Self {
        Self::new_truncate(0)
    }

    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as _
    }

    pub const fn as_ptr_mut<T>(self) -> *mut T {
        self.0 as _
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn offset(self) -> usize {
        self.0 & Self::OFFSET_MASK
    }

    pub const fn table_indices(self) -> [usize; 4] {
        let m = Self::INDEX_MASK;
        [
            (self.0 >> 39) & m,
            (self.0 >> 30) & m,
            (self.0 >> 21) & m,
            (self.0 >> 12) & m,
        ]
    }
}

impl Add for VirtAddr {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.0.checked_add(rhs.0).unwrap())
    }
}

impl Add<usize> for VirtAddr {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self::new(self.0.checked_add(rhs).unwrap())
    }
}

impl AddAssign for VirtAddr {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.add(rhs);
    }
}

impl AddAssign<usize> for VirtAddr {
    fn add_assign(&mut self, rhs: usize) {
        *self = self.add(rhs);
    }
}

impl fmt::Display for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "v{:#x}", self.0)
    }
}

//

#[derive(Debug)]
pub struct InvalidAddress;

//

pub const fn align_up(addr: usize, align: usize) -> usize {
    assert!(align.is_power_of_two(), "align has to be a power of 2");
    let mask = align - 1;

    if addr & mask == 0 {
        addr
    } else {
        (addr | mask).checked_add(1).expect("align_up overflow")
    }
}

pub const fn align_down(addr: usize, align: usize) -> usize {
    assert!(align.is_power_of_two(), "align has to be a power of 2");
    let mask = align - 1;
    addr & !mask
}

pub const fn is_aligned(addr: usize, align: usize) -> bool {
    assert!(align.is_power_of_two(), "align has to be a power of 2");
    addr % align == 0
}
