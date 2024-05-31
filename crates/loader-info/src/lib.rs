#![no_std]

//

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LoaderInfo {
    device_tree_blob: *const u8,
}
