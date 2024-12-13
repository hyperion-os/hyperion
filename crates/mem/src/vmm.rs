use core::{
    fmt,
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

use bitflags::bitflags;
use x86_64::{structures::paging::PageTableFlags, PhysAddr, VirtAddr};

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privilege {
    User,
    Kernel,
}

/// inversed to make `?` more useful
///
/// TODO: impl try
pub type PageFaultResult = Result<NotHandled, Handled>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handled;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotHandled;

//

#[derive(Debug)]
pub struct MemoryInfo {
    /// mapped virtual memory in pages `0x1000` (excluding the higher half)
    ///
    /// includes memory that is not yet mapped
    pub virt_pages: AtomicUsize,

    /// mapped physical memory in pages `0x1000` (excluding the higher half)
    pub phys_pages: AtomicUsize,

    pub id: usize,
}

impl MemoryInfo {
    pub fn zero() -> Self {
        Self::symmetric(0)
    }

    pub fn symmetric(n: usize) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        Self {
            virt_pages: AtomicUsize::new(n),
            phys_pages: AtomicUsize::new(n),
            id,
        }
    }

    // FIXME: Relaxed ordering?

    pub fn add_virt(&self, n_pages: usize) {
        self.virt_pages.fetch_add(n_pages, Ordering::Acquire);
    }

    pub fn add_phys(&self, n_pages: usize) {
        self.phys_pages.fetch_add(n_pages, Ordering::Acquire);
    }

    pub fn sub_virt(&self, n_pages: usize) {
        if self.virt_pages.fetch_sub(n_pages, Ordering::Release) == 0 {
            panic!("double free detected");
        }
    }

    pub fn sub_phys(&self, n_pages: usize) {
        if self.phys_pages.fetch_sub(n_pages, Ordering::Release) == 0 {
            panic!("double free detected");
        }
    }

    /// vm bytes
    pub fn virt_size(&self) -> usize {
        self.virt_pages.load(Ordering::Relaxed) * 0x1000
    }

    /// pm bytes
    pub fn phys_size(&self) -> usize {
        self.phys_pages.load(Ordering::Relaxed) * 0x1000
    }
}

//

#[derive(Debug, Clone, Copy)]
pub enum MapTarget {
    /// pages are mapped immediately but the VMM is not allowed to free them
    Borrowed(PhysAddr),

    /// pages are mapped immediately and the VMM is allowed to free them
    Preallocated(PhysAddr),

    /// pages are allocated and mapped lazily
    LazyAlloc,
}

impl MapTarget {
    pub fn inc_addr(&mut self, by: u64) {
        match self {
            MapTarget::Borrowed(a) | MapTarget::Preallocated(a) => *a += by,
            MapTarget::LazyAlloc => {}
        }
    }

    pub fn is_aligned(&self, to: u64) -> bool {
        match self {
            MapTarget::Borrowed(a) | MapTarget::Preallocated(a) => a.is_aligned(to),
            MapTarget::LazyAlloc => true,
        }
    }
}

impl fmt::Display for MapTarget {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MapTarget::Borrowed(a) | MapTarget::Preallocated(a) => write!(f, "{a:#018x}"),
            MapTarget::LazyAlloc => write!(f, "<lazy-alloc>"),
        }
    }
}

//

bitflags! {
pub struct MapFlags: u8 {
    const WRITE   = 0b0000_0001;
    const NO_EXEC = 0b0000_0010;
    const USER    = 0b0000_0100;
}
}

//

pub trait PageMapImpl {
    /// handle a page fault, possibly related to lazy mapping or CoW pages
    fn page_fault(&self, v_addr: VirtAddr, privilege: Privilege) -> PageFaultResult;

    /// take the current virtual address space
    fn current() -> Self;

    /// create a new virtual address space
    fn new() -> Self;

    /// statistics on virt/phys memory allocations
    fn info(&self) -> &MemoryInfo;

    /// lazy clone this virtual address space
    fn fork(&self) -> Self;

    /// switch to this virtual address space
    fn activate(&self);

    /// convert virtual addr to physical addr, by reading the page tables
    fn virt_to_phys(&self, v_addr: VirtAddr) -> Option<PhysAddr>;

    /// convert physical addr to virtual addr, by moving it to the higher half
    fn phys_to_virt(&self, p_addr: PhysAddr) -> VirtAddr;

    /// get an address where anything can be mapped (temporarily)
    fn temporary(index: u16) -> VirtAddr;

    /// map address temporarily somewhere
    fn map_temporary(
        &mut self,
        info: &MemoryInfo,
        to: PhysAddr,
        bytes: usize,
        flags: PageTableFlags,
    ) -> VirtAddr;

    /// unmap a previously temporarily mapped
    fn unmap_temporary(&mut self, info: &MemoryInfo, from: VirtAddr);

    /// map physical memory into virtual memory
    ///
    /// `p_addr` None means that the pages need to be allocated (possibly lazily on use)
    fn map(&self, v_addr: Range<VirtAddr>, p_addr: MapTarget, flags: PageTableFlags);

    /// unmap a range of virtual memory
    fn unmap(&self, v_addr: Range<VirtAddr>);

    /// remap the pages with new flags but the same physical memory
    fn remap(&self, v_addr: Range<VirtAddr>, new_flags: PageTableFlags);

    /// test if a virtual memory range is mapped with (at least) the given flags
    fn is_mapped(&self, v_addr: Range<VirtAddr>, has_at_least: PageTableFlags) -> bool;
}
