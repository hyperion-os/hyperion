use crate::debug;
use spin::{Lazy, Once};
use x86_64::{
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
    GDT_ONCE.call_once(|| GDT.0.load());

    unsafe {
        CS::set_reg(GDT.1.kc);
        SS::set_reg(GDT.1.kd);
        // load_tss(GDT.1.tss);
    }

    debug!("correct gdt={:?}", GDT);
}

//

#[derive(Debug)]
struct SegmentSelectors {
    kc: SegmentSelector,
    kd: SegmentSelector,
    // tss: SegmentSelector,
}

static GDT: Lazy<(GlobalDescriptorTable, SegmentSelectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    let sel = SegmentSelectors {
        kc: gdt.add_entry(Descriptor::kernel_code_segment()),
        kd: gdt.add_entry(Descriptor::kernel_data_segment()),
        // tss: gdt.add_entry(Descriptor::tss_segment(&TSS)),
    };
    // gdt.add_entry(Descriptor::user_code_segment());
    // gdt.add_entry(Descriptor::user_data_segment());
    (gdt, sel)
});
static GDT_ONCE: Once<()> = Once::new();

static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[0] = {
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
