use core::{
    fmt,
    mem::{self, MaybeUninit},
    ptr::copy_nonoverlapping,
    slice,
    sync::atomic::{AtomicU16, AtomicUsize, Ordering},
};

use log::println;
use riscv64_vmm::{align_up, PhysAddr};
use spin::Once;
use util::{prefix::NumberFmt, rle::RleMemoryRef};

//

pub const PAGE_SIZE: usize = 0x1000;
pub const ZEROED_PAGE: PageType = [0; U64_PER_PAGE];

type PageType = [u64; U64_PER_PAGE];
const U64_PER_PAGE: usize = PAGE_SIZE / mem::size_of::<u64>();

pub static FRAME_ALLOC: Once<FrameAllocator> = Once::new();

//

pub fn init(memory: &RleMemoryRef) {
    FRAME_ALLOC.call_once(|| FrameAllocator::init(memory));
}

/// Alloc pages
///
/// Use [`Self::free`] to not leak pages (-> memory)
pub fn alloc() -> Frame {
    allocator().alloc()
}

/// Free up pages
pub fn free(frame: Frame) {
    allocator().free(frame)
}

/// Free up pages without destroying the data
pub fn free_no_overwrite(frame: Frame) {
    allocator().free_no_overwrite(frame)
}

/// mark a page as shared (or make a copy if it if there are too many refs)
///
/// # Safety
/// the pages should not be modified or deallocated during the copy
///
/// the original and copied pages shouldn't be written to
pub unsafe fn fork(frame: Frame) -> Frame {
    unsafe { allocator().fork(frame) }
}

/// handle a page fault from a forked CoW page
///
/// # Internal
///
/// if the page has 0 refs, calling this panics
///
/// if the page has 1 ref, it is now exclusive and can just be made writeable
///
/// if the page has 2 or more refs,
/// the ref count is decremented and a copy is made and that copy is returned
///
/// # Safety
/// `mapped` should point to `frame` in the active page mapper
pub unsafe fn page_fault(frame: Frame) -> Frame {
    unsafe { allocator().fork_page_fault(frame) }
}

/// System total memory in bytes
pub fn total_mem() -> usize {
    try_allocator().map_or(0, FrameAllocator::total_mem)
}

/// System usable memory in bytes
pub fn usable_mem() -> usize {
    try_allocator().map_or(0, FrameAllocator::usable_mem)
}

/// Currently used usable memory in bytes
pub fn used_mem() -> usize {
    try_allocator().map_or(0, FrameAllocator::used_mem)
}

/// Currently free usable memory in bytes
pub fn free_mem() -> usize {
    try_allocator().map_or(0, FrameAllocator::free_mem)
}

/// Reserved memory in bytes
pub fn reserved_mem() -> usize {
    try_allocator().map_or(0, FrameAllocator::reserved_mem)
}

pub fn allocator() -> &'static FrameAllocator {
    FRAME_ALLOC.get().expect("frame allocator not initialized")
}

pub fn try_allocator() -> Option<&'static FrameAllocator> {
    FRAME_ALLOC.get()
}

//

pub struct Frame {
    addr: PhysAddr,
}

impl Frame {
    pub const unsafe fn new(addr: PhysAddr) -> Self {
        Self { addr }
    }

    pub const unsafe fn from_idx(idx: usize) -> Self {
        unsafe { Self::new(PhysAddr::new(idx * PAGE_SIZE)) }
    }

    pub const fn page_index(&self) -> usize {
        self.addr.as_usize() / PAGE_SIZE
    }

    pub const fn addr(&self) -> PhysAddr {
        self.addr
    }
}

//

pub struct FrameAllocator {
    // some metadata for every single usable memory page
    // for things like copy on write and things
    pages: &'static [PageInfo],
    first_page: usize,

    // some memory statistics
    usable: AtomicUsize,
    used: AtomicUsize,
    total: AtomicUsize,

    // optimization for the next alloc address
    last_alloc_index: AtomicUsize,
}

impl FrameAllocator {
    /// System total memory in bytes
    pub fn total_mem(&self) -> usize {
        self.total.load(Ordering::SeqCst)
    }

    /// System usable memory in bytes
    pub fn usable_mem(&self) -> usize {
        self.usable.load(Ordering::SeqCst)
    }

    /// Currently used usable memory in bytes
    pub fn used_mem(&self) -> usize {
        self.used.load(Ordering::SeqCst)
    }

    /// Currently free usable memory in bytes
    pub fn free_mem(&self) -> usize {
        self.usable_mem() - self.used_mem()
    }

    /// Reserved memory in bytes
    pub fn reserved_mem(&self) -> usize {
        self.total_mem() - self.usable_mem()
    }

    /// # Safety
    ///
    /// this is safe to call once the bootloader memory is guaranteed to not be used anymore
    ///
    /// so after the bootloader stacks are freed and the bootloader page mapper is freed
    /// and there are no calls to things like Limine requests
    ///
    /// I use Lazy in limine requests to avoid reading the raw data twice, so most Limine
    /// requests should be already cached, and 'should be' is admittedly not 'guaranteed'
    pub unsafe fn free_bootloader(&self) {
        // TODO:
    }

    /// Free up pages
    pub fn free(&self, frame: Frame) {
        let data = unsafe { &mut *frame.addr().to_hhdm().as_ptr_mut::<MaybeUninit<_>>() };
        data.write(ZEROED_PAGE);

        self.free_no_overwrite(frame);
    }

    /// Free up pages without destroying the data
    pub fn free_no_overwrite(&self, frame: Frame) {
        let page = frame.page_index() - self.first_page;

        if self.pages[page].free() {
            // race conditions in statistics don't matter
            self.used.fetch_sub(PAGE_SIZE, Ordering::Release);
        }
    }

    /// mark a page as shared (or make a copy if it if there are too many refs)
    ///
    /// # Safety
    /// the pages should not be modified or deallocated during the copy
    ///
    /// the original and copied pages shouldn't be written to
    pub unsafe fn fork(&self, frame: Frame) -> Frame {
        let page = frame.page_index() - self.first_page;

        if matches!(unsafe { self.pages[page].copy() }, Err(TooManyRefs)) {
            unsafe { self.cold_copy_fork(frame) }
        } else {
            frame
        }
    }

    #[cold]
    unsafe fn cold_copy_fork(&self, frame: Frame) -> Frame {
        let copy = self.alloc();

        unsafe {
            copy_nonoverlapping::<PageType>(
                frame.addr().to_hhdm().as_ptr(),
                copy.addr().to_hhdm().as_ptr_mut(),
                1,
            );
        }

        copy
    }

    /// handle a page fault from a forked CoW page
    ///
    /// # Internal
    ///
    /// if the page has 0 refs, calling this panics
    ///
    /// if the page has 1 ref, it is now exclusive and can just be made writeable
    ///
    /// if the page has 2 or more refs,
    /// the ref count is decremented and a copy is made and that copy is returned
    ///
    /// # Safety
    /// `mapped` should point to `frame` in the active page mapper
    pub unsafe fn fork_page_fault(&self, frame: Frame) -> Frame {
        let page = frame.page_index() - self.first_page;
        let ref_count = &self.pages[page].ref_count;

        match ref_count.load(Ordering::Acquire) {
            0 => {
                // trying to fork a free page
                panic!()
            }
            1 => {
                // exclusive access
                frame
            }
            mut other => {
                // make a copy of the original page,
                // before giving up the ref
                let copy = unsafe { self.cold_copy_fork(frame) };

                loop {
                    // decrement the ref count and copy the page
                    // fetch_sub won't work because if the second 'owner' frees it
                    // (right after the load above),
                    // the ref count becomes 1 and the fetch_sub would mark the page as free,
                    // which is obviously bad
                    if ref_count
                        .compare_exchange(other, other - 1, Ordering::Release, Ordering::Relaxed)
                        .is_ok()
                    {
                        if other == 1 {
                            // 2 copies were made and the original got deallocated
                            // (a small waste of time but idc)
                            self.used.fetch_sub(PAGE_SIZE, Ordering::Release);
                        }

                        // a copy has been made and the original page is now
                        // either deallocated or shared between some other process(es)
                        return copy;
                    } else {
                        // it doesnt matter if the ref count goes to 1 here
                        // because the copy has already been made
                        other = ref_count.load(Ordering::Acquire);
                    }
                }
            }
        }
    }

    /// Alloc pages
    ///
    /// Use [`Self::free`] to not leak pages (-> memory)
    pub fn alloc(&self) -> Frame {
        for _ in 0..self.pages.len() {
            // try allocating up to self.pages.len() times

            let at = self.last_alloc_index.fetch_add(1, Ordering::Relaxed);
            let idx = at % self.pages.len();
            let page = &self.pages[idx];

            if !page.alloc() {
                continue;
            }

            // zeroing shouldn't be needed,
            // all memory given to the frame allocator should already be safe to allocate
            // without any zeroing and all freed memory that needs to be zeroed, gets zeroed
            // during the free
            return unsafe { Frame::from_idx(self.first_page + idx) };
        }

        panic!("OOM");
    }

    pub fn init(memory: &RleMemoryRef) -> Self {
        // usable system memory
        let mut usable = memory.iter_usable().map(|r| r.size.get()).sum::<usize>();

        // total system memory
        let total: usize = memory.max_usable_addr(); // FIXME: should be max (not usable) addr

        let usable_pages = (memory.max_usable_addr() - memory.min_usable_addr()) / PAGE_SIZE;
        let metadata_block_size = align_up(usable_pages * mem::size_of::<PageInfo>(), PAGE_SIZE);
        usable -= metadata_block_size;

        let metadata_block_region = memory
            .iter_usable()
            .find(|r| r.size.get() >= metadata_block_size)
            .expect("Not enough contiguous memory");
        let metadata_block_addr = PhysAddr::new(metadata_block_region.addr).to_hhdm();

        let metadata_block: &mut [MaybeUninit<PageInfo>] =
            unsafe { slice::from_raw_parts_mut(metadata_block_addr.as_ptr_mut(), usable_pages) };

        metadata_block.fill_with(|| {
            MaybeUninit::new(PageInfo {
                ref_count: AtomicU16::new(1), // mark all pages as used
            })
        });

        let pages = unsafe { MaybeUninit::slice_assume_init_ref(metadata_block) };

        let pfa = Self {
            pages,
            first_page: memory.min_usable_addr() / PAGE_SIZE,

            usable: usable.into(),
            used: usable.into(),
            total: total.into(),

            last_alloc_index: 0.into(),
        };

        // free up some pages
        println!(
            "frame allocator uses {}B to track {}B pages",
            metadata_block_size.binary(),
            usable.binary()
        );
        for usable in memory.iter_usable() {
            let mut addr = usable.addr;
            let mut size = usable.size.get();

            if addr == metadata_block_region.addr {
                addr += metadata_block_size;
                size -= metadata_block_size;
            }

            if size == 0 {
                continue;
            }

            for idx in 0..size / PAGE_SIZE {
                let idx = addr / PAGE_SIZE + idx;
                let frame = unsafe { Frame::from_idx(idx) };

                pfa.free(frame);
            }
        }

        println!("frame allocator initialized:\n{pfa}");

        pfa
    }
}

impl fmt::Display for FrameAllocator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        const M: &str = "system memory";
        writeln!(f, "Total    {M}: {}B", self.total_mem().binary())?;
        writeln!(f, "Usable   {M}: {}B", self.usable_mem().binary())?;
        writeln!(f, "Used     {M}: {}B", self.used_mem().binary())?;
        writeln!(f, "Free     {M}: {}B", self.free_mem().binary())?;
        write!(f, "Reserved {M}: {}B", self.reserved_mem().binary())?;

        Ok(())
    }
}
//

#[derive(Debug, Clone, Copy)]
pub struct TooManyRefs;

//

pub struct PageInfo {
    ref_count: AtomicU16,
}

impl PageInfo {
    // `true` = alloc successful
    fn alloc(&self) -> bool {
        self.ref_count
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// # Safety
    /// this page should not be deallocated during this clone, it can be cloned though
    unsafe fn copy(&self) -> Result<(), TooManyRefs> {
        // TODO: orderings
        self.ref_count
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| old.checked_add(1))
            .map(|_| {})
            .map_err(|_| TooManyRefs)
    }

    /// returns true if the page is actually free to allocate now
    fn free(&self) -> bool {
        match self.ref_count.fetch_sub(1, Ordering::Release) {
            0 => panic!("double free detected"),
            1 => true,
            _ => false,
        }
    }
}
