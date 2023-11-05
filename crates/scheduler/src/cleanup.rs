use core::{ptr::NonNull, sync::atomic::AtomicUsize};

use hyperion_instant::Instant;

use crate::{futex, ipc, schedule, sleep, task::Task};

//

pub struct CleanupTask {
    task: Task,
    cleanup: Cleanup,
}

impl CleanupTask {
    pub fn run(self) {
        self.cleanup.run(self.task);
    }
}

//

#[derive(Debug, Clone, Copy)]
pub enum Cleanup {
    Sleep {
        deadline: Instant,
    },
    // Lock,
    Wait {
        addr: NonNull<AtomicUsize>,
        val: usize,
    },
    SimpleIpcWait,
    Drop,
    Ready,
}

impl Cleanup {
    pub const fn task(self, task: Task) -> CleanupTask {
        CleanupTask {
            task,
            cleanup: self,
        }
    }

    pub fn run(self, task: Task) {
        match self {
            Self::Sleep { deadline } => sleep::push(deadline, task),
            // Self::Lock => lock::cleanup(task),
            Self::Wait { addr, val } => futex::cleanup(addr, val, task),
            Self::SimpleIpcWait => ipc::start_waiting(task),
            Self::Drop => {}
            Self::Ready => {
                schedule(task);
            }
        }
    }
}

unsafe impl Send for Cleanup {}
