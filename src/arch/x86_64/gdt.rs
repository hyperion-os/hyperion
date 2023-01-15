use super::idt::DOUBLE_FAULT_IST;
use crate::debug;
use spin::Lazy;
use x86_64::{
    instructions::tables::load_tss,
    registers::segmentation::{Segment, CS, SS},
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
    VirtAddr,
};

//

pub fn init() {
    debug!("Initializing GDT");
    GDT.0.load();

    unsafe {
        CS::set_reg(GDT.1.kc);
        SS::set_reg(GDT.1.kd);
        load_tss(GDT.1.tss);
    }
}

//

struct SegmentSelectors {
    kc: SegmentSelector,
    kd: SegmentSelector,
    tss: SegmentSelector,
}

static GDT: Lazy<(GlobalDescriptorTable, SegmentSelectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    let sel = SegmentSelectors {
        kc: gdt.add_entry(Descriptor::kernel_code_segment()),
        kd: gdt.add_entry(Descriptor::kernel_data_segment()),
        tss: gdt.add_entry(Descriptor::tss_segment(&TSS)),
    };
    // gdt.add_entry(Descriptor::user_code_segment());
    // gdt.add_entry(Descriptor::user_data_segment());
    (gdt, sel)
});

static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[DOUBLE_FAULT_IST as usize] = {
        static mut STACK: [u8; 4096 * 5] = [0; 4096 * 5];

        let stack_range = unsafe { STACK }.as_ptr_range();
        VirtAddr::from_ptr(stack_range.end)
    };
    tss.privilege_stack_table[0] = {
        static mut STACK: [u8; 4096 * 5] = [0; 4096 * 5];

        let stack_range = unsafe { STACK }.as_ptr_range();
        VirtAddr::from_ptr(stack_range.end)
    };
    tss
});
