#![no_std]
#![feature(atomic_from_mut, const_mut_refs)]

//

use core::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Default)]
pub struct Bitmap<'a> {
    data: &'a mut [u8],
}

//

impl<'a> Bitmap<'a> {
    #[must_use]
    pub const fn new(data: &'a mut [u8]) -> Self {
        Self { data }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.data.len() * 8
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // pub fn resize(&mut self, bits: usize) {
    //     let bytes = bits / 8 + 1;
    //     self.data.resize(bytes, 0);
    // }

    pub fn fill(&mut self, val: bool) {
        self.data.fill(if val { 0xFF } else { 0x0 });
    }

    #[must_use]
    pub fn get(&self, n: usize) -> Option<bool> {
        let (byte, _, mask) = bp(n);
        let byte = *self.data.get(byte)?;

        Some(byte & mask != 0)
    }

    #[must_use]
    pub fn set(&mut self, n: usize, val: bool) -> Option<()> {
        let (byte, bit, mask) = bp(n);
        let byte = self.data.get_mut(byte)?;

        // reset the bit
        *byte &= !mask;

        // set the bit
        *byte |= u8::from(val) << bit;

        Some(())
    }

    /// iterator over indexes of 1 bits
    pub fn iter_true(&self) -> impl Iterator<Item = usize> + '_ {
        self.iter_bytes()
            .enumerate()
            .filter(|(_, byte)| *byte != 0)
            .flat_map(|(i, byte)| {
                (0..8)
                    .enumerate()
                    .filter(move |(_, bit)| byte & (1 << *bit) != 0)
                    .map(move |(j, _)| i * 8 + j)
            })
    }

    /// iterator over indexes of 0 bits
    pub fn iter_false(&self) -> impl Iterator<Item = usize> + '_ {
        self.iter_bytes()
            .enumerate()
            .filter(|(_, byte)| *byte != 0xFF)
            .flat_map(|(i, byte)| {
                (0..8)
                    .enumerate()
                    .filter(move |(_, bit)| byte & (1 << *bit) == 0)
                    .map(move |(j, _)| i * 8 + j)
            })
    }

    pub fn iter(&self) -> impl Iterator<Item = bool> + '_ {
        self.iter_bytes()
            .flat_map(|byte| (0..8).map(move |i| byte & (1 << i) != 0))
    }

    pub fn iter_bytes(&self) -> impl Iterator<Item = u8> + '_ {
        self.data.iter().copied()
    }
}

//

pub struct AtomicBitmap<'a> {
    data: &'a [AtomicU8],
}

impl<'a> AtomicBitmap<'a> {
    #[must_use]
    pub const fn new(data: &'a [AtomicU8]) -> Self {
        Self { data }
    }

    #[must_use]
    pub fn from_mut(data: &'a mut [u8]) -> Self {
        Self::new(AtomicU8::from_mut_slice(data))
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.data.len() * 8
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn fill(&self, val: bool, order: Ordering) {
        let val = if val { 0xFF } else { 0x0 };
        for b in self.data {
            b.store(val, order);
        }
    }

    #[must_use]
    pub fn load(&self, n: usize, order: Ordering) -> Option<bool> {
        let (byte, _, mask) = bp(n);
        let byte = self.data.get(byte)?;

        Some(byte.load(order) & mask != 0)
    }

    #[must_use]
    pub fn store(&self, n: usize, val: bool, order: Ordering) -> Option<()> {
        let (byte, _, mask) = bp(n);
        let byte = self.data.get(byte)?;

        if val {
            byte.fetch_or(mask, order);
        } else {
            byte.fetch_and(!mask, order);
        }

        Some(())
    }

    #[must_use]
    pub fn swap(&self, n: usize, val: bool, order: Ordering) -> Option<bool> {
        let (byte, _, mask) = bp(n);
        let byte = self.data.get(byte)?;

        let old = if val {
            byte.fetch_or(mask, order)
        } else {
            byte.fetch_and(!mask, order)
        };

        Some(old & mask != 0)
    }
}

impl<'a> From<&'a mut [u8]> for AtomicBitmap<'a> {
    fn from(value: &'a mut [u8]) -> Self {
        Self::from_mut(value)
    }
}

impl<'a, const N: usize> From<&'a mut [u8; N]> for AtomicBitmap<'a> {
    fn from(value: &'a mut [u8; N]) -> Self {
        Self::from_mut(value)
    }
}

//

/// byte index, bit index, bitmask
#[must_use]
fn bp(n: usize) -> (usize, usize, u8) {
    let bit = n % 8;
    (n / 8, bit, 1 << bit)
}

//

#[cfg(test)]
mod tests {
    use core::sync::atomic::Ordering;

    use super::Bitmap;
    use crate::AtomicBitmap;

    #[test]
    fn test_bitmap_iter_true() {
        let mut bitmap = [0; 10];
        let mut bitmap = Bitmap::new(&mut bitmap);

        assert_eq!(bitmap.set(5, true), Some(()));
        assert_eq!(bitmap.set(7, true), Some(()));
        assert_eq!(bitmap.set(9, true), Some(()));
        assert_eq!(bitmap.set(53, true), Some(()));
        assert_eq!(bitmap.set(79, true), Some(()));
        assert_eq!(bitmap.set(89, true), None);

        let mut iter = bitmap.iter_true();
        assert_eq!(iter.next(), Some(5));
        assert_eq!(iter.next(), Some(7));
        assert_eq!(iter.next(), Some(9));
        assert_eq!(iter.next(), Some(53));
        assert_eq!(iter.next(), Some(79));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_bitmap_iter_false() {
        let mut bitmap = [0xFF; 10];
        let mut bitmap = Bitmap::new(&mut bitmap);
        assert_eq!(bitmap.set(5, false), Some(()));
        assert_eq!(bitmap.set(7, false), Some(()));
        assert_eq!(bitmap.set(9, false), Some(()));
        assert_eq!(bitmap.set(53, false), Some(()));
        assert_eq!(bitmap.set(79, false), Some(()));
        assert_eq!(bitmap.set(89, false), None);

        let mut iter = bitmap.iter_false();
        assert_eq!(iter.next(), Some(5));
        assert_eq!(iter.next(), Some(7));
        assert_eq!(iter.next(), Some(9));
        assert_eq!(iter.next(), Some(53));
        assert_eq!(iter.next(), Some(79));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_atomic_bitmap() {
        let mut bitmap = [0xFF; 10];
        let bitmap = AtomicBitmap::from(&mut bitmap);

        assert_eq!(bitmap.load(5, Ordering::SeqCst), Some(true));
        assert_eq!(bitmap.swap(5, false, Ordering::SeqCst), Some(true));
        assert_eq!(bitmap.swap(5, false, Ordering::SeqCst), Some(false));
        assert_eq!(bitmap.load(5, Ordering::SeqCst), Some(false));

        assert_eq!(bitmap.load(7, Ordering::SeqCst), Some(true));
        assert_eq!(bitmap.swap(7, false, Ordering::SeqCst), Some(true));
        assert_eq!(bitmap.swap(7, false, Ordering::SeqCst), Some(false));
        assert_eq!(bitmap.load(7, Ordering::SeqCst), Some(false));

        assert_eq!(bitmap.load(9, Ordering::SeqCst), Some(true));
        assert_eq!(bitmap.swap(9, false, Ordering::SeqCst), Some(true));
        assert_eq!(bitmap.swap(9, false, Ordering::SeqCst), Some(false));
        assert_eq!(bitmap.load(9, Ordering::SeqCst), Some(false));

        assert_eq!(bitmap.swap(89, false, Ordering::SeqCst), None);
    }
}
