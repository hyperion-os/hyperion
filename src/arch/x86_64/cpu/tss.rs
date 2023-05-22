use core::sync::atomic::{AtomicBool, Ordering};

use x86_64::{structures::tss::TaskStateSegment, VirtAddr};

//

pub struct Tss {
    pub inner: TaskStateSegment,
    pub stacks: TssStacks,
}

pub struct TssStacks {
    interrupt: [AtomicBool; 7],
    // privilege: [bool; 3],
}

//

impl Tss {
    pub fn new() -> Self {
        let mut tss = Self {
            inner: TaskStateSegment::new(),
            stacks: TssStacks {
                interrupt: [(); 7].map(|_| AtomicBool::new(false)),
                // privilege: [false; 3],
            },
        };

        static mut INT_STACK_0: [u8; 4096 * 5] = [0; 4096 * 5];
        tss.add_int(1, unsafe { &mut INT_STACK_0 });

        // static mut PRIV_STACK_0: [u8; 4096 * 5] = [0; 4096 * 5];
        // tss.add_priv(0, unsafe { &mut PRIV_STACK_0 });

        tss
    }

    fn add_int(&mut self, idx: usize, stack: &'static mut [u8]) {
        self.inner.interrupt_stack_table[idx] = VirtAddr::from_ptr(stack.as_ptr_range().end);
        self.stacks.interrupt[idx].store(true, Ordering::SeqCst);
    }

    // fn add_priv(&mut self, stacks: &mut TssStacks, idx: usize, stack: &'static mut [u8]) {
    //     self.inner.privilege_stack_table[idx] = VirtAddr::from_ptr(stack.as_ptr_range().end);
    //     stacks.privilege[idx] = true;
    // }
}

impl Default for Tss {
    fn default() -> Self {
        Self::new()
    }
}

impl TssStacks {
    pub fn take_interrupt_stack(&self) -> Option<u16> {
        self.interrupt
            .iter()
            .enumerate()
            .find_map(|(idx, avail)| avail.swap(false, Ordering::SeqCst).then_some(idx as u16))
    }
}
