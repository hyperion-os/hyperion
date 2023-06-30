use hyperion_log::debug;
use hyperion_mem::to_higher_half;
use x86_64::{
    structures::paging::{PageTable, PageTableFlags},
    PhysAddr,
};

/// PML4 (Level 4 Page Map)
///
/// 64 bit CR3 page (requires PAE)
#[derive(Debug)]
pub struct Level4<'a>(pub &'a PageTable);

/// PDP (Page Directory Pointer)
///
/// 32 bit CR3 page
#[derive(Debug)]
pub struct Level3<'a>(pub &'a PageTable);

/// PD (Page Directory)
///
/// skipped in 1GiB pages
#[derive(Debug)]
pub struct Level2<'a>(pub &'a PageTable);

/// PT (Page Table)
///
/// skipped in 2 MiB and 1GiB pages
#[derive(Debug)]
pub struct Level1<'a>(pub &'a PageTable);

pub enum WalkTableResult {
    Size1GiB(PhysAddr),
    Size2MiB(PhysAddr),
    Size4KiB(PhysAddr),
    FrameNotPresent,
}

pub enum WalkTableIterResult<'a> {
    Size1GiB(PhysAddr),
    Size2MiB(PhysAddr),
    Size4KiB(PhysAddr),
    Level3(Level3<'a>),
    Level2(Level2<'a>),
    Level1(Level1<'a>),
}

//

impl<'a> Level4<'a> {
    pub const fn from_pml4(table: &'a PageTable) -> Self {
        Self(table)
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, PageTableFlags, WalkTableIterResult)> + 'a {
        iter_table(self.0).map(|(i, is_huge, flags, addr)| {
            if is_huge {
                panic!("512 GiB pages are not supported");
            }

            debug!("L4       {i} = {:?}", flags);

            let res = WalkTableIterResult::Level3(Level3(get_table(addr)));
            (i, flags, res)
        })
    }
}

impl<'a> Level3<'a> {
    pub fn iter(&self) -> impl Iterator<Item = (usize, PageTableFlags, WalkTableIterResult)> + 'a {
        iter_table(self.0).map(|(i, is_huge, flags, addr)| {
            if is_huge {
                // debug!("      L3 HUGE {i} = {:?}", flags);
                return (i, flags, WalkTableIterResult::Size1GiB(addr));
            }

            debug!("  L3     {i} = {:?}", flags);

            let res = WalkTableIterResult::Level2(Level2(get_table(addr)));
            (i, flags, res)
        })
    }
}

impl<'a> Level2<'a> {
    pub fn iter(&self) -> impl Iterator<Item = (usize, PageTableFlags, WalkTableIterResult)> + 'a {
        iter_table(self.0).map(|(i, is_huge, flags, addr)| {
            if is_huge {
                // debug!("    L2 HUGE  {i} = {:?}", flags);
                return (i, flags, WalkTableIterResult::Size2MiB(addr));
            }

            debug!("    L2   {i} = {:?}", flags);

            let res = WalkTableIterResult::Level1(Level1(get_table(addr)));
            (i, flags, res)
        })
    }
}

impl<'a> Level1<'a> {
    pub fn iter(&self) -> impl Iterator<Item = (usize, PageTableFlags, WalkTableIterResult)> + 'a {
        iter_table(self.0).map(|(i, is_huge, flags, addr)| {
            if is_huge {
                panic!("L1 cannot be huge");
            }

            // debug!("      L1 {i} = {:?}", flags);

            let res = WalkTableIterResult::Size4KiB(addr);
            (i, flags, res)
        })
    }
}

//

fn iter_table(
    table: &PageTable,
) -> impl Iterator<Item = (usize, bool, PageTableFlags, PhysAddr)> + '_ {
    table
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.flags().contains(PageTableFlags::PRESENT))
        .map(move |(i, entry)| {
            (
                i,
                entry.flags().contains(PageTableFlags::HUGE_PAGE),
                entry.flags(),
                entry.addr(),
            )
        })
}

fn get_table(addr: PhysAddr) -> &'static PageTable {
    unsafe { &*to_higher_half(addr).as_ptr() }
}
