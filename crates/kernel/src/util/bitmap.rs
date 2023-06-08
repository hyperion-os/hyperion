#[derive(Debug, Default)]
pub struct Bitmap<'a> {
    data: &'a mut [u8],
}

//

impl<'a> Bitmap<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len() * 8
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // pub fn resize(&mut self, bits: usize) {
    //     let bytes = bits / 8 + 1;
    //     self.data.resize(bytes, 0);
    // }

    pub fn fill(&mut self, val: bool) {
        self.data.fill(if val { 0xFF } else { 0x0 })
    }

    #[must_use]
    pub fn get(&self, n: usize) -> Option<bool> {
        let (byte, _, mask) = self.bp(n);
        let byte = *self.data.get(byte)?;

        Some(byte & mask != 0)
    }

    #[must_use]
    pub fn set(&mut self, n: usize, val: bool) -> Option<()> {
        let (byte, bit, mask) = self.bp(n);
        let byte = self.data.get_mut(byte)?;

        // reset the bit
        *byte &= !mask;

        // set the bit
        *byte |= (val as u8) << bit;

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

    #[must_use]
    fn bp(&self, n: usize) -> (usize, usize, u8) {
        let bit = n % 8;
        (n / 8, bit, 1 << bit)
    }
}

//

#[cfg(test)]
mod tests {
    use super::Bitmap;

    #[test_case]
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

    #[test_case]
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
}
