use core::{
    fmt,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageFaultResult {
    Handled,
    NotHandled,
}

impl PageFaultResult {
    pub fn is_handled(self) -> bool {
        self == Self::Handled
    }
}

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

    /// pages are set to guard mode
    NeverMap,
}

impl MapTarget {
    pub fn inc_addr(&mut self, by: u64) {
        match self {
            MapTarget::Borrowed(a) | MapTarget::Preallocated(a) => *a += by,
            MapTarget::LazyAlloc | MapTarget::NeverMap => {}
        }
    }

    pub fn is_aligned(&self, to: u64) -> bool {
        match self {
            MapTarget::Borrowed(a) | MapTarget::Preallocated(a) => a.is_aligned(to),
            MapTarget::LazyAlloc | MapTarget::NeverMap => true,
        }
    }
}

impl fmt::Display for MapTarget {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MapTarget::Borrowed(a) | MapTarget::Preallocated(a) => write!(f, "{a:#018x}"),
            MapTarget::LazyAlloc => write!(f, "<lazy-alloc>"),
            MapTarget::NeverMap => write!(f, "<guard>"),
        }
    }
}

//

bitflags! {
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MapFlags: u8 {
    /// non-read pages cant write or execute either,
    /// they are basically guard pages
    const READ    = 0b0001;
    const WRITE   = 0b0010;
    const EXEC    = 0b0100;
    const USER    = 0b1000;
    const NONE    = 0b0000;
}
}

impl fmt::Display for MapFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use core::fmt::Write;
        f.write_char(if self.contains(Self::READ) { 'R' } else { '-' })?;
        f.write_char(if self.contains(Self::WRITE) { 'W' } else { '-' })?;
        f.write_char(if self.contains(Self::EXEC) { 'X' } else { '-' })?;
        f.write_char(if self.contains(Self::USER) { 'U' } else { '-' })?;
        Ok(())
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

    /// print the address space to logs
    fn debug(&self);

    /// statistics on virt/phys memory allocations
    fn info(&self) -> &MemoryInfo;

    /// lazy clone this virtual address space
    fn fork(&self) -> Self;

    /// switch to this virtual address space
    fn activate(&self);

    /// convert virtual addr to physical addr, by reading the page tables
    fn virt_to_phys(&self, v_addr: VirtAddr) -> Option<(PhysAddr, PageTableFlags)>;

    /// map things into virtual memory
    fn map(&self, addr: VirtAddr, len: usize, to: MapTarget, flags: MapFlags);

    // FIXME: use after free race condition, because of TLB cache, if other CPUs arent stopped
    /// unmap a range of virtual memory
    fn unmap(&self, addr: VirtAddr, len: usize);

    // FIXME: use after free race condition, because of TLB cache, if other CPUs arent stopped
    /// remap the pages with new flags but the same physical memory
    fn remap(&self, addr: VirtAddr, len: usize, new_flags: MapFlags);

    /// test if a virtual memory range is mapped with (at least) the given flags
    fn is_mapped(&self, addr: VirtAddr, len: usize, has_at_least: MapFlags) -> bool;
}

/* pub struct Temporary<'a, P: PageMapImpl> {
    /// l3 index in the 510th l4 entry
    pub index: u16,
    /// 1GiB virtual memory area that is free to use
    pub v_addr: Range<VirtAddr>,

    map: &'a P,
}

impl<P: PageMapImpl> Drop for Temporary<'_, P> {
    fn drop(&mut self) {
        self.map.free_temporary();
        todo!()
    }
} */
