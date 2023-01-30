#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Memmap {
    pub base: u64,
    pub len: u64,
}

//

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    #[test_case]
    fn test_alloc() {
        core::hint::black_box((0..64).map(|i| i * 2).collect::<Vec<_>>());
    }
}
