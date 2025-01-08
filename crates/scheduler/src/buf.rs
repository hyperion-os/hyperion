use alloc::sync::Arc;
use core::{
    marker::PhantomData,
    ops::{Deref, Range},
    ptr,
};

use hyperion_arch::vmm::{PageMap, NO_FREE};
use hyperion_mem::vmm::{MapTarget, PageMapImpl};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

use crate::proc::Process;

//

/// mutable buffer inside of some process
pub struct BufferMut<'a> {
    inner: Buffer<'a>,
}

impl<'a> BufferMut<'a> {
    pub fn from_kernel(slice: &'a mut [u8]) -> Self {
        Self {
            inner: Buffer::from_kernel(slice),
        }
    }

    pub fn copy_from(&mut self, src: &Buffer) {
        let src_beg = VirtAddr::from_ptr(src.ptr.as_ptr());
        let src_end = src_beg + src.ptr.len() as u64;

        page_split_iterator(src_beg..src_end);
    }
}

impl BufferMut<'static> {
    pub fn from_proc(proc: Arc<Process>, ptr_arg: u64, len_arg: u64) -> Self {
        Self {
            inner: Buffer::from_proc(proc, ptr_arg, len_arg),
        }
    }
}

impl<'a> Deref for BufferMut<'a> {
    type Target = Buffer<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

//

/// buffer inside of some process
pub struct Buffer<'a> {
    proc: Option<Arc<Process>>,
    ptr: *const [u8],
    _p: PhantomData<&'a ()>,
}

impl<'a> Buffer<'a> {
    pub fn from_kernel(slice: &'a [u8]) -> Self {
        Self {
            proc: None,
            ptr: slice as _,
            _p: PhantomData,
        }
    }

    fn map(&self, active: &PageMap, idx: &mut u16) -> *const [u8] {
        let Some(proc) = self.proc.as_ref() else {
            // kernel buffers are always mapped, because kernel space is global
            return self.ptr;
        };

        let index = *idx;
        *idx += 1;

        let v_addr = PageMap::temporary(index);

        // TODO: this could be more optimal
        let beg = VirtAddr::from_ptr(self.ptr.as_ptr());
        let end = VirtAddr::from_ptr((self.ptr.as_ptr() as usize + self.ptr.len()) as *const u8);

        let aligned_beg = beg.align_down(0x1000u64);
        let aligned_end = end.align_up(0x1000u64);

        let n_pages = aligned_end - aligned_beg;

        for i in 0..n_pages {
            let v_addr_src = aligned_beg + i * 0x1000;
            let v_addr_dst = v_addr + i * 0x1000;
            let Some((p_addr, flags)) = proc.address_space.virt_to_phys(v_addr_src) else {
                continue;
            };

            active.map(
                v_addr_dst..v_addr_src + 1,
                MapTarget::Borrowed(p_addr),
                flags,
            );
        }

        let buffer_beg = v_addr + (beg - aligned_beg);
        ptr::slice_from_raw_parts(buffer_beg.as_ptr(), self.ptr.len())
    }
}

impl Buffer<'static> {
    pub fn from_proc(proc: Arc<Process>, ptr_arg: u64, len_arg: u64) -> Self {
        Self {
            proc: Some(proc),
            ptr: ptr::slice_from_raw_parts(ptr_arg as usize as _, len_arg as usize),
            _p: PhantomData,
        }
    }
}

//

/// splits the `buf` in page boundaries
pub fn page_split_iterator(buf: Range<VirtAddr>) -> impl Iterator<Item = Range<VirtAddr>> {
    let beg = buf.start;
    let end = buf.end;

    let aligned_beg = beg.align_up(0x1000u64);
    let aligned_end = end.align_down(0x1000u64);

    let first_size = aligned_end - beg;
    let last_size = end - aligned_end;

    let n_full_pages = (aligned_end - aligned_beg) / 0x1000;

    (first_size != 0)
        .then_some(beg..aligned_beg)
        .into_iter()
        .chain((0..n_full_pages).map(move |i| {
            let page = aligned_beg + i * 0x1000;
            page..page + 0x1000
        }))
        .chain((last_size != 0).then_some(aligned_end..end))
}
