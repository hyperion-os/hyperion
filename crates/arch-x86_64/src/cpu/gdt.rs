use x86_64::{
    instructions::tables::load_tss,
    registers::segmentation::{Segment, CS, SS},
    structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
};

use super::tss::Tss;

//

#[derive(Debug)]
pub struct Gdt {
    inner: GlobalDescriptorTable,
    pub selectors: SegmentSelectors,
}

#[derive(Debug, Clone, Copy)]
pub struct SegmentSelectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
    tss: SegmentSelector,
}

//

impl Gdt {
    pub fn new(tss: &'static Tss) -> Self {
        let mut inner = GlobalDescriptorTable::new();

        let selectors = SegmentSelectors {
            kernel_code: inner.append(Descriptor::kernel_code_segment()),
            kernel_data: inner.append(Descriptor::kernel_data_segment()),
            user_data: inner.append(Descriptor::user_data_segment()),
            user_code: inner.append(Descriptor::user_code_segment()),
            tss: inner.append(Descriptor::tss_segment(unsafe { &*tss.inner.get() })),
        };

        Self { inner, selectors }
    }

    pub fn load(&'static self) {
        // trace!("Loading GDT");
        self.inner.load();

        unsafe {
            CS::set_reg(self.selectors.kernel_code);
            SS::set_reg(self.selectors.kernel_data);
            load_tss(self.selectors.tss);
        }
    }
}
