use alloc::{
    boxed::Box,
    format,
    string::String,
    sync::{Arc, Weak},
};
use core::{
    any::Any,
    fmt::{self, Display, Write},
    sync::atomic::Ordering,
};

use arcstr::ArcStr;
use hyperion_mem::vmm::PageMapImpl;
use hyperion_scheduler::{
    proc::{processes, Pid, Process, PROCESSES},
    process,
};
use hyperion_syscall::err::{Error, Result};
use hyperion_vfs::{
    device::{ArcOrRef, DirEntry, DirectoryDevice, FileDevice},
    tree::{IntoNode, Node},
};

use crate::process_ext_with;

//

mod sysctl;

//

pub fn init(root: impl IntoNode) {
    root.into_node().mount("proc", ProcFs::new());
}

//

pub struct ProcFs {
    // TODO: doesnt have to be sync, use Option instead of Once
    cmdline: Option<Node>,
    version: Option<Node>,
    cpuinfo: Option<Node>,
    sys: Option<Node>,
}

impl ProcFs {
    pub const fn new() -> Self {
        Self {
            cmdline: None,
            version: None,
            cpuinfo: None,
            sys: None,
        }
    }

    fn meminfo(&self) -> Node {
        let pfa = &*hyperion_mem::pmm::PFA;

        // create a snapshot of the system memory info to fix some data races
        Node::new_file(DisplayFile(MemInfo {
            total: pfa.usable_mem() / 0x400,
            free: pfa.free_mem() / 0x400,
        }))
    }

    fn cmdline(&mut self) -> Node {
        self.cmdline
            .get_or_insert_with(|| Node::new_file(DisplayFile(hyperion_boot::args::get().cmdline)))
            .clone()
    }

    fn version(&mut self) -> Node {
        self.version
            .get_or_insert_with(|| {
                Node::new_file(DisplayFile(format!(
                    "{} version {} #{} {}",
                    hyperion_kernel_info::NAME,
                    hyperion_kernel_info::VERSION,
                    hyperion_kernel_info::BUILD_REV,
                    hyperion_kernel_info::BUILD_TIME,
                )))
            })
            .clone()
    }

    fn uptime(&mut self) -> Node {
        Node::new_file(DisplayFile(Uptime {
            system_s: (hyperion_clock::get().nanosecond_now() / 10_000_000) as f32 / 100.0,
            cpu_idle_sum_s: hyperion_scheduler::idle()
                .map(|cpu_idle| cpu_idle.as_seconds_f32())
                .sum::<f32>(),
        }))
    }

    fn cpuinfo(&mut self) -> Node {
        self.cpuinfo
            .get_or_insert_with(|| {
                let mut buf = String::new();
                for n in 0..hyperion_boot::cpu_count() {
                    _ = writeln!(&mut buf, "processor : {n}");
                    _ = writeln!(&mut buf);
                }
                Node::new_file(DisplayFile(buf))
            })
            .clone()
    }

    fn self_dir(&self) -> Node {
        Node::new_dir(ProcDir(process()))
    }

    fn sys(&mut self) -> Node {
        self.sys
            .get_or_insert_with(|| sysctl::sysctl_base())
            .clone()
    }
}

impl DirectoryDevice for ProcFs {
    fn driver(&self) -> &'static str {
        "procfs"
    }

    fn get_node(&mut self, name: &str) -> Result<Node> {
        match name {
            "meminfo" => Ok(self.meminfo()),
            "cmdline" => Ok(self.cmdline()),
            "version" => Ok(self.version()),
            "uptime" => Ok(self.uptime()),
            "cpuinfo" => Ok(self.cpuinfo()),
            "self" => Ok(self.self_dir()),
            "sys" => Ok(self.sys()),
            _ => {
                if let Some(proc) = name.parse::<usize>().ok().and_then(|pid| {
                    let processes = PROCESSES.lock();
                    processes.get(&Pid::new(pid)).and_then(Weak::upgrade)
                }) {
                    return Ok(Node::new_dir(ProcDir(proc)));
                }

                Err(Error::NOT_FOUND)
            }
        }
    }

    fn nodes(&mut self) -> Result<Box<dyn ExactSizeIterator<Item = DirEntry<'_>> + '_>> {
        struct ExactSizeChain<A, B>(A, B);

        impl<A: Iterator, B: Iterator<Item = A::Item>> Iterator for ExactSizeChain<A, B> {
            type Item = A::Item;

            fn next(&mut self) -> Option<Self::Item> {
                if let Some(v) = self.0.next() {
                    return Some(v);
                }

                self.1.next()
            }
        }

        impl<A: ExactSizeIterator, B: ExactSizeIterator<Item = A::Item>> ExactSizeIterator
            for ExactSizeChain<A, B>
        {
            fn len(&self) -> usize {
                self.0.len() + self.1.len()
            }
        }

        Ok(Box::new(ExactSizeChain(
            [
                ("cmdline", self.cmdline()),
                ("cpuinfo", self.cpuinfo()),
                ("meminfo", self.meminfo()),
                ("uptime", self.uptime()),
                ("version", self.version()),
                ("self", self.self_dir()),
                ("sys", self.sys()),
            ]
            .into_iter()
            .map(|(name, node)| DirEntry {
                name: ArcOrRef::Ref(name),
                node,
            }),
            processes().into_iter().map(|s| DirEntry {
                name: ArcOrRef::Arc(format!("{}", s.pid).into()),
                node: Node::new_dir(ProcDir(s)),
            }),
        )))
    }

    // fn nodes(&mut self) -> IoResult<Arc<[Arc<str>]>> {
    //     static FILES: Lazy<Arc<[Arc<str>]>> = Lazy::new(|| {
    //         [
    //             "cmdline".into(),
    //             "cpuinfo".into(),
    //             "meminfo".into(),
    //             "uptime".into(),
    //             "version".into(),
    //         ]
    //         .into()
    //     });

    //     Ok(FILES.clone())
    // }
}

//

struct ProcDir(Arc<Process>);

impl ProcDir {
    fn status(&self) -> Node {
        Node::new_file(DisplayFile(ProcStatus::new(self.0.clone())))
    }

    fn cmdline(&self) -> Node {
        if let Some(cmdline) = process_ext_with(&self.0).cmdline.get().cloned() {
            Node::new_file(DisplayFile(cmdline))
        } else {
            Node::new_file(DisplayFile(self.0.name.read().clone()))
        }
    }
}

impl DirectoryDevice for ProcDir {
    fn get_node(&mut self, name: &str) -> Result<Node> {
        match name {
            "status" => Ok(self.status()),
            "cmdline" => Ok(self.cmdline()),
            _ => Err(Error::NOT_FOUND),
        }
    }

    fn nodes(&mut self) -> Result<Box<dyn ExactSizeIterator<Item = DirEntry<'_>> + '_>> {
        Ok(Box::new(
            [
                DirEntry {
                    name: ArcOrRef::Ref("status"),
                    node: self.status(),
                },
                DirEntry {
                    name: ArcOrRef::Ref("cmdline"),
                    node: self.cmdline(),
                },
            ]
            .into_iter(),
        ))
    }
}

//

struct ProcStatus {
    name: ArcStr,
    pid: Pid,
    threads: usize,
    nanos: u64,
    vm_size: u64,
}

impl ProcStatus {
    pub fn new(proc: Arc<Process>) -> Self {
        Self {
            name: proc.name.read().clone(),
            pid: proc.pid,
            threads: proc.threads.load(Ordering::Relaxed),
            nanos: proc.nanos.load(Ordering::Relaxed),
            vm_size: proc.address_space.page_map.info().virt_size() as u64 >> 10,
        }
    }
}

impl fmt::Display for ProcStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Name: {}", self.name)?;
        writeln!(f, "Pid: {}", self.pid)?;
        writeln!(f, "Threads: {}", self.threads)?;
        writeln!(f, "Nanos: {}", self.nanos)?;
        writeln!(f, "VmSize: {} kB", self.vm_size)?;
        Ok(())
    }
}

//

struct MemInfo {
    total: usize,
    free: usize,
}

impl fmt::Display for MemInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "MemTotal: {} kb", self.total)?;
        writeln!(f, "MemFree:  {} kb", self.free)?;
        Ok(())
    }
}

//

struct Uptime {
    system_s: f32,
    cpu_idle_sum_s: f32,
}

impl fmt::Display for Uptime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{:.2} {:.2}", self.system_s, self.cpu_idle_sum_s)?;
        Ok(())
    }
}

//

struct DisplayFile<T>(T);

impl<T: Display + Send + Sync + 'static> FileDevice for DisplayFile<T> {
    fn driver(&self) -> &'static str {
        "procfs"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        let mut l = FmtLength { len: 0 };
        write!(&mut l, "{}", self.0).unwrap();
        l.len
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let mut w = FmtOffsetBuf {
            offset,
            buf,
            cursor: 0,
        };
        write!(&mut w, "{}", self.0).unwrap();
        Ok(w.cursor.saturating_sub(w.offset).min(w.buf.len()))
    }
}

//

struct FmtLength {
    len: usize,
}

impl fmt::Write for FmtLength {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.len += s.len();
        Ok(())
    }
}

struct FmtOffsetBuf<'a> {
    // 0..offset | offset..offset+buf.len() | offset+buf.len()..
    //   ignored | written to buf           | ignored
    offset: usize,
    buf: &'a mut [u8],
    cursor: usize,
}

impl fmt::Write for FmtOffsetBuf<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let s = s.as_bytes();

        // write s into buf when s starts from self.cursor and buf starts from offset
        // both have some virtual empty space before and after
        //
        // so it is like an unaligned slice copy
        //           +--------+
        //           |abc     |
        //           +--------+
        //             |
        //            \/
        //      +--------+
        //      |     abc|
        //      +--------+
        if let (Some(s), Some(buf)) = (
            s.get(self.offset.saturating_sub(self.cursor)..),
            self.buf.get_mut(self.cursor.saturating_sub(self.offset)..),
        ) {
            let limit = s.len().min(buf.len());
            buf[..limit].copy_from_slice(&s[..limit]);
        }

        self.cursor += s.len();

        Ok(())
    }
}
