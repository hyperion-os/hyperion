#![no_std]

//

use core::{
    any::type_name,
    arch::asm,
    fmt,
    ops::{Add, AddAssign},
};

use util::{align_down, align_up, is_aligned};

//

/// HCF instruction
pub fn halt_and_catch_fire() -> ! {
    loop {
        wait_for_interrupts();
    }
}

/// WFI instruction
pub extern "C" fn wait_for_interrupts() {
    unsafe {
        asm!("wfi");
    }
}

//

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

    pub const fn abs_diff(self, other: Self) -> usize {
        self.0.abs_diff(other.0)
    }

    /// align up to a 4KiB page
    pub const fn align_up(self) -> Self {
        self.align_up_to(1 << 12)
    }

    /// align down to a 4KiB page
    pub const fn align_down(self) -> Self {
        self.align_down_to(1 << 12)
    }

    /// is aligned to a 4KiB page?
    pub const fn is_aligned(self) -> bool {
        self.is_aligned_to(1 << 12)
    }

    pub const fn align_up_to(self, align: usize) -> Self {
        Self::new(align_up(self.0, align))
    }

    pub const fn align_down_to(self, align: usize) -> Self {
        Self::new(align_down(self.0, align))
    }

    pub const fn is_aligned_to(self, align: usize) -> bool {
        is_aligned(self.as_usize(), align)
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

impl fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple(type_name::<Self>())
            .field(&format_args!("p{:#x}", self.0))
            .finish()
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
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

    pub const fn abs_diff(self, other: Self) -> usize {
        self.0.abs_diff(other.0)
    }

    /// align up to a 4KiB page
    pub const fn align_up(self) -> Self {
        self.align_up_to(1 << 12)
    }

    /// align down to a 4KiB page
    pub const fn align_down(self) -> Self {
        self.align_down_to(1 << 12)
    }

    /// is aligned to a 4KiB page?
    pub const fn is_aligned(self) -> bool {
        self.is_aligned_to(1 << 12)
    }

    pub const fn align_up_to(self, align: usize) -> Self {
        Self::new(align_up(self.0, align))
    }

    pub const fn align_down_to(self, align: usize) -> Self {
        Self::new(align_down(self.0, align))
    }

    pub const fn is_aligned_to(self, align: usize) -> bool {
        is_aligned(self.as_usize(), align)
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

impl fmt::Debug for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple(type_name::<Self>())
            .field(&format_args!("p{:#x}", self.0))
            .finish()
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
