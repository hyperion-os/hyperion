#![no_std]
#![feature(iter_map_windows)]

//

extern crate alloc;

use core::{mem, ops::Range};

use hyperion_arch::vmm::PageMap;
use hyperion_mem::{to_higher_half, vmm::PageMapImpl};
use hyperion_syscall::err::{Error, Result};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub fn is_user_accessible(
    page_map: &PageMap,
    ptr: u64,
    len: u64,
    has_at_least: PageTableFlags,
) -> Result<(VirtAddr, usize)> {
    if len == 0 {
        return Ok((VirtAddr::new_truncate(0), 0));
    }

    let Some(end) = ptr.checked_add(len) else {
        return Err(Error::INVALID_ADDRESS);
    };

    let (Ok(start), Ok(end)) = (VirtAddr::try_new(ptr), VirtAddr::try_new(end)) else {
        return Err(Error::INVALID_ADDRESS);
    };

    if !page_map.is_mapped(start..end, has_at_least) {
        // debug!("{:?} not mapped", start..end);
        return Err(Error::INVALID_ADDRESS);
    }

    Ok((start, len as _))
}

pub fn read_slice_parts(ptr: u64, len: u64) -> Result<(VirtAddr, usize)> {
    is_user_accessible(
        &PageMap::current(),
        ptr,
        len,
        PageTableFlags::USER_ACCESSIBLE,
    )
}

pub fn read_slice_parts_mut(ptr: u64, len: u64) -> Result<(VirtAddr, usize)> {
    is_user_accessible(
        &PageMap::current(),
        ptr,
        len,
        PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE,
    )
}

/// use physical memory to read item(s) from an inactive process
pub fn phys_read_item_from_proc<T: Copy>(page_map: &PageMap, ptr: u64, to: &mut [T]) -> Result<()> {
    phys_read_from_proc(page_map, ptr, unsafe {
        &mut *core::ptr::slice_from_raw_parts_mut(
            to.as_mut_ptr().cast(),
            mem::size_of::<T>() * to.len(),
        )
    })
}

/// use physical memory to write item(s) into an inactive process
pub fn phys_write_item_into_proc<T: Copy>(page_map: &PageMap, ptr: u64, from: &[T]) -> Result<()> {
    phys_write_into_proc(page_map, ptr, unsafe {
        &*core::ptr::slice_from_raw_parts(from.as_ptr().cast(), mem::size_of::<T>() * from.len())
    })
}

/// use physical memory to read byte(s) from an inactive process
pub fn phys_read_from_proc(page_map: &PageMap, ptr: u64, mut to: &mut [u8]) -> Result<()> {
    let len = to.len() as u64;
    let (start, len) = is_user_accessible(page_map, ptr, len, PageTableFlags::USER_ACCESSIBLE)?;

    let mut now;
    for (base, len) in split_pages(start..start + len) {
        // copy one page at a time
        let hhdm = to_higher_half(page_map.virt_to_phys(base).unwrap());
        let from: &[u8] = unsafe { &*core::ptr::slice_from_raw_parts(hhdm.as_ptr(), len as usize) };

        (now, to) = to.split_at_mut(len as usize);
        now.copy_from_slice(from);
    }

    Ok(())
}

/// use physical memory to write byte(s) into an inactive process
pub fn phys_write_into_proc(page_map: &PageMap, ptr: u64, mut from: &[u8]) -> Result<()> {
    let len = from.len() as u64;
    let (start, len) = is_user_accessible(
        page_map,
        ptr,
        len,
        PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE,
    )?;

    let mut now;
    for (base, len) in split_pages(start..start + len) {
        // copy one page at a time
        let hhdm = to_higher_half(page_map.virt_to_phys(base).unwrap());
        let to: &mut [u8] =
            unsafe { &mut *core::ptr::slice_from_raw_parts_mut(hhdm.as_mut_ptr(), len as usize) };

        (now, from) = from.split_at(len as usize);
        to.copy_from_slice(now);
    }

    Ok(())
}

/// iterate through all pages that contain this range
pub fn split_pages(range: Range<VirtAddr>) -> impl Iterator<Item = (VirtAddr, u16)> {
    let start_idx = range.start.as_u64() / 0x1000;
    let end_idx = range.end.as_u64() / 0x1000;

    core::iter::once(range.start)
        .chain((start_idx..end_idx).map(|idx| VirtAddr::new(idx * 0x1000)))
        .chain(core::iter::once(range.end))
        .map_windows(|[a, b]| (*a, (b.as_u64() - a.as_u64()) as u16))
}

pub fn read_untrusted_ref<'a, T>(ptr: u64) -> Result<&'a T> {
    if !(ptr as *const T).is_aligned() {
        return Err(Error::INVALID_ADDRESS);
    }

    read_slice_parts(ptr, mem::size_of::<T>() as _).map(|(start, _)| unsafe { &*start.as_ptr() })
}

pub fn read_untrusted_mut<'a, T>(ptr: u64) -> Result<&'a mut T> {
    if !(ptr as *const T).is_aligned() {
        hyperion_log::debug!("not aligned");
        return Err(Error::INVALID_ADDRESS);
    }

    read_slice_parts_mut(ptr, mem::size_of::<T>() as _)
        .map(|(start, _)| unsafe { &mut *start.as_mut_ptr() })
}

pub fn read_untrusted_slice<'a, T: Copy>(ptr: u64, len: u64) -> Result<&'a [T]> {
    read_slice_parts(ptr, len).map(|(start, len)| {
        // TODO:
        // SAFETY: this is most likely unsafe
        if len == 0 {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(start.as_ptr(), len as _) }
        }
    })
}

pub fn read_untrusted_bytes<'a>(ptr: u64, len: u64) -> Result<&'a [u8]> {
    read_slice_parts(ptr, len).map(|(start, len)| {
        // TODO:
        // SAFETY: this is most likely unsafe
        if len == 0 {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(start.as_ptr(), len as _) }
        }
    })
}

pub fn read_untrusted_bytes_mut<'a>(ptr: u64, len: u64) -> Result<&'a mut [u8]> {
    read_slice_parts_mut(ptr, len).map(|(start, len)| {
        // TODO:
        // SAFETY: this is most likely unsafe
        if len == 0 {
            &mut []
        } else {
            unsafe { core::slice::from_raw_parts_mut(start.as_mut_ptr(), len as _) }
        }
    })
}

pub fn read_untrusted_str<'a>(ptr: u64, len: u64) -> Result<&'a str> {
    read_untrusted_bytes(ptr, len)
        .and_then(|bytes| core::str::from_utf8(bytes).map_err(|_| Error::INVALID_UTF8))
}
