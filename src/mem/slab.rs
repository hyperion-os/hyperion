use super::{
    pmm::{self},
    to_higher_half,
};
use core::{mem, slice};
use spin::RwLock;
use x86_64::{align_up, VirtAddr};

//

pub struct SlabAllocator {
    slabs: [(RwLock<Slab>, usize); 7],
}

pub struct Slab {
    idx: u8,
    size: usize,

    next: Option<VirtAddr>,
}

pub struct SlabHeader {
    slab_idx: u8,
}

pub struct SlabData {
    next: Option<VirtAddr>,
}

//

impl SlabAllocator {
    pub fn new() -> Self {
        let mut idx = 0u8;
        Self {
            slabs: [8, 16, 32, 64, 128, 256, 512].map(|size| {
                let slab_idx = idx;
                idx += 1;

                (RwLock::new(Slab::new(slab_idx, size)), size)
            }),
        }
    }

    pub fn get_slab(&self, size: usize) -> Option<&RwLock<Slab>> {
        self.slabs
            .iter()
            .find(|(_, slab_size)| *slab_size >= size)
            .map(|(slab, _)| slab)
    }

    pub fn alloc(&self, size: usize) -> VirtAddr {
        self.get_slab(size)
            .expect("TODO: Big alloc")
            .write()
            .alloc()
    }

    pub fn free(&self, v_addr: VirtAddr) {
        let header = v_addr.align_down(4096u64);
        let header: &mut SlabHeader = unsafe { &mut *header.as_mut_ptr() };

        self.slabs[header.slab_idx as usize].0.write().free(v_addr);
    }
}

impl Default for SlabAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl Slab {
    pub fn new(idx: u8, size: usize) -> Self {
        Self {
            idx,
            size,
            next: None,
        }
    }

    pub fn next_slab(&mut self) -> VirtAddr {
        *self.next.get_or_insert_with(|| {
            let page = pmm::PageFrameAllocator::get().alloc(1);
            let page_bytes = page.byte_len();
            let page = to_higher_half(page.addr());

            // write header

            let header: &mut SlabHeader = unsafe { &mut *page.as_mut_ptr() };
            header.slab_idx = self.idx;

            // write slab chain

            let header_align = align_up(mem::size_of::<SlabHeader>() as u64, self.size as _);

            let len = (page_bytes - header_align as usize) / mem::size_of::<SlabData>();
            let data: &mut [SlabData] =
                unsafe { slice::from_raw_parts_mut((page + header_align).as_mut_ptr(), len) };
            let step = self.size / mem::size_of::<SlabData>();

            for (prev, next) in (0..len - 1).zip(1..len).step_by(step) {
                let next_addr = Some(VirtAddr::new(&data[next] as *const SlabData as u64));
                data[prev].next = next_addr;
            }
            if let Some(last) = data.iter_mut().step_by(step).last() {
                last.next = None
            }

            page
        })
    }

    pub fn alloc(&mut self) -> VirtAddr {
        let slab = self.next_slab();

        let step = self.size / mem::size_of::<SlabData>();
        let data: &mut [SlabData] = unsafe { slice::from_raw_parts_mut(slab.as_mut_ptr(), step) };
        self.next = data[0].next;

        data.fill_with(|| unsafe { mem::zeroed() });

        slab
    }

    pub fn free(&mut self, slab: VirtAddr) {
        if slab.as_u64() == 0 {
            return;
        }

        let step = self.size / mem::size_of::<SlabData>();
        let data: &mut [SlabData] = unsafe { slice::from_raw_parts_mut(slab.as_mut_ptr(), step) };
        data[0].next = self.next;
        self.next = Some(slab);
    }
}
