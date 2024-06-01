#![no_std]

//

use util::rle::Segment;

//

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LoaderInfo {
    pub device_tree_blob: *const u8,
    pub memory: *const [Segment],
}
