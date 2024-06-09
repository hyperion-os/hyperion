use core::arch::asm;

//

pub struct Satp;

impl Satp {
    pub fn read() -> usize {
        let satp;
        unsafe { asm!("csrr {satp}, satp", satp = out(reg) satp) };
        satp
    }

    /// # Safety
    /// `satp` has to be valid, and everything currently in use
    /// should be mapped correctly to the page table
    pub unsafe fn write(satp: usize) {
        unsafe { asm!("csrw satp, {satp}", satp = in(reg) satp) };
    }
}
