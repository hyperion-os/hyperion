use core::num::NonZero;

use heapless::Vec;

//

/// run-length encoded memory regions
///
/// the last segment is always usable,
/// because everything is reserved by default
#[derive(Debug, Clone)]
pub struct RleMemory {
    // TODO: this could be placed into memory right after the loader,
    // and then only the used region can be reserved
    segments: Vec<Segment, 64>,
}

impl RleMemory {
    pub const fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub fn as_slice(&self) -> &[Segment] {
        &self.segments
    }

    pub fn min_usable_addr(&self) -> usize {
        let Some(first) = self.segments.first() else {
            return 0;
        };

        if first.ty == SegmentType::Reserved {
            first.size.get()
        } else {
            0
        }
    }

    pub fn max_usable_addr(&self) -> usize {
        self.end_addr()
    }

    pub fn iter_usable(&self) -> impl Iterator<Item = Region> + '_ {
        self.iter_segments()
            .filter(|(_, ty)| *ty == SegmentType::Usable)
            .map(|(reg, _)| reg)
    }

    pub fn iter_segments(&self) -> impl Iterator<Item = (Region, SegmentType)> + '_ {
        self.segments.iter().scan(0usize, |acc, segment| {
            let now = *acc;
            *acc += segment.size.get();
            Some((
                Region {
                    addr: now,
                    size: segment.size,
                },
                segment.ty,
            ))
        })
    }

    pub fn end_addr(&self) -> usize {
        self.iter_segments().last().map_or(0, |(last_region, ty)| {
            assert_eq!(ty, SegmentType::Usable);
            last_region.addr + last_region.size.get()
        })
    }

    /// add usable memory
    pub fn insert(&mut self, region: Region) {
        self.insert_segment_at(region, SegmentType::Usable);
    }

    /// reserve unusable memory
    pub fn remove(&mut self, region: Region) {
        self.insert_segment_at(region, SegmentType::Reserved);
    }

    // TODO: these allocations cannot be freed,
    // because it gets treated the same as hw reserved memory,
    // which will be reserved forever
    /// reserve the first usable 4K and get the address
    pub fn alloc(&mut self) -> usize {
        let mut addr = 0usize;
        let Some(mut first) = self.segments.first_mut() else {
            return 0;
        };

        if first.ty == SegmentType::Reserved {
            addr = first.size.get();
            first = &mut self.segments[1];
            assert_eq!(first.ty, SegmentType::Usable);
        }

        // TODO (assert): all memory region sizes should be multiples of (1 << 12)
        let left = first.size.get() - (1 << 12);
        if let Some(size) = NonZero::new(left) {
            first.size = size;
            self.segments
                .insert(
                    0,
                    Segment {
                        size: unsafe { NonZero::new_unchecked(1 << 12) },
                        ty: SegmentType::Reserved,
                    },
                )
                .expect("memory too segmented");
        } else {
            first.ty = SegmentType::Reserved;
        }

        self.fixup();

        addr
    }

    fn insert_segment_at(&mut self, region: Region, region_ty: SegmentType) {
        let new_addr = region.addr;
        let new_end_addr = new_addr + region.size.get();

        let mut current_segment_addr = 0usize;
        let mut i = 0usize;
        while i < self.segments.len() {
            let segment = self.segments[i];
            let current_segment_end_addr = current_segment_addr + segment.size.get();

            if new_addr >= current_segment_end_addr {
                // no overlaps, continue
                i += 1;
                current_segment_addr += segment.size.get();
                continue;
            } else if current_segment_addr >= new_end_addr {
                // the segment is already past the new region, so no more overlaps can come
                current_segment_addr += segment.size.get();
                break;
            } else if segment.ty == region_ty {
                // both segments are the same, so it is technically already merged
                i += 1;
                current_segment_addr += segment.size.get();
                continue;
            }

            // overlap detected, split the original one into up to 3 pieces

            let segment_split_left_size = new_addr.saturating_sub(current_segment_addr);
            let segment_split_right_size = current_segment_end_addr.saturating_sub(new_end_addr);

            // FIXME: remove(i) followed by insert(i)
            self.segments.remove(i);

            if let Some(size) = NonZero::new(segment_split_left_size) {
                self.segments
                    .insert(
                        i,
                        Segment {
                            size,
                            ty: segment.ty,
                        },
                    )
                    .expect("memory is too segmented");
                i += 1;
            }
            if let Some(size) = NonZero::new(
                segment.size.get() - segment_split_left_size - segment_split_right_size,
            ) {
                self.segments
                    .insert(
                        i,
                        Segment {
                            size,
                            ty: region_ty,
                        },
                    )
                    .expect("memory is too segmented");
                i += 1;
            }
            if let Some(size) = NonZero::new(segment_split_right_size) {
                self.segments
                    .insert(
                        i,
                        Segment {
                            size,
                            ty: segment.ty,
                        },
                    )
                    .expect("memory is too segmented");
                i += 1;
            }

            current_segment_addr += segment.size.get();
        }

        if region_ty == SegmentType::Usable {
            if let Some(leftover) = new_end_addr
                .checked_sub(new_addr.max(current_segment_addr))
                .and_then(NonZero::new)
            {
                if let Some(padding) = new_addr
                    .checked_sub(current_segment_addr)
                    .and_then(NonZero::new)
                {
                    self.segments
                        .push(Segment {
                            size: padding,
                            ty: SegmentType::Reserved,
                        })
                        .expect("memory is too segmented");
                }

                self.segments
                    .push(Segment {
                        size: leftover,
                        ty: SegmentType::Usable,
                    })
                    .expect("memory is too segmented");
            }
        }

        // FIXME: shouln't be needed
        self.fixup();
    }

    fn fixup(&mut self) {
        // FIXME: there shouldnt be any Reserved entries in the end
        if let Some(Segment {
            ty: SegmentType::Reserved,
            ..
        }) = self.segments.last()
        {
            self.segments.pop();
        }

        // FIXME: all segments should already be merged
        let mut i = 0usize;
        while i + 1 < self.segments.len() {
            let right = self.segments[i + 1];
            let left = &mut self.segments[i];

            if left.ty == right.ty {
                left.size = left.size.checked_add(right.size.get()).unwrap();
                self.segments.remove(i + 1);
            } else {
                i += 1;
            }
        }
    }
}

impl Default for RleMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Region {
    pub addr: usize,
    pub size: NonZero<usize>,
}

/// one piece of run-length encoded memory regions
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Segment {
    // FIXME: idk y, but rustc cannot squeeze SegmentType and NonZero<usize> into one u64
    // the same way as it can squeeze Option<NonZero<usize>> into one u64
    pub size: NonZero<usize>,
    pub ty: SegmentType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum SegmentType {
    Reserved,
    Usable,
}
