use x86_64::PhysAddr;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Memmap {
    pub base: PhysAddr,
    pub len: u64,
    pub ty: Memtype,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Memtype {
    Usable,
    BootloaderReclaimable,
    KernelAndModules,
    Framebuffer,
}

//

impl Memmap {
    /// Returns `true` if the memtype is [`Usable`].
    ///
    /// [`Usable`]: Memtype::Usable
    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.ty.is_usable()
    }

    /// Returns `true` if the memtype is [`BootloaderReclaimable`].
    ///
    /// [`BootloaderReclaimable`]: Memtype::BootloaderReclaimable
    #[must_use]
    pub fn is_bootloader_reclaimable(&self) -> bool {
        self.ty.is_bootloader_reclaimable()
    }

    /// Returns `true` if the memtype is [`KernelAndModules`].
    ///
    /// [`KernelAndModules`]: Memtype::KernelAndModules
    #[must_use]
    pub fn is_kernel_and_modules(&self) -> bool {
        self.ty.is_kernel_and_modules()
    }

    /// Returns `true` if the memtype is [`Framebuffer`].
    ///
    /// [`Framebuffer`]: Memtype::Framebuffer
    #[must_use]
    pub fn is_framebuffer(&self) -> bool {
        self.ty.is_framebuffer()
    }
}

impl Memtype {
    /// Returns `true` if the memtype is [`Usable`].
    ///
    /// [`Usable`]: Memtype::Usable
    #[must_use]
    pub fn is_usable(&self) -> bool {
        matches!(self, Self::Usable)
    }

    /// Returns `true` if the memtype is [`BootloaderReclaimable`].
    ///
    /// [`BootloaderReclaimable`]: Memtype::BootloaderReclaimable
    #[must_use]
    pub fn is_bootloader_reclaimable(&self) -> bool {
        matches!(self, Self::BootloaderReclaimable)
    }

    /// Returns `true` if the memtype is [`KernelAndModules`].
    ///
    /// [`KernelAndModules`]: Memtype::KernelAndModules
    #[must_use]
    pub fn is_kernel_and_modules(&self) -> bool {
        matches!(self, Self::KernelAndModules)
    }

    /// Returns `true` if the memtype is [`Framebuffer`].
    ///
    /// [`Framebuffer`]: Memtype::Framebuffer
    #[must_use]
    pub fn is_framebuffer(&self) -> bool {
        matches!(self, Self::Framebuffer)
    }
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
