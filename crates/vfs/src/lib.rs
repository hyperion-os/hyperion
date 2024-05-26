#![no_std]

//

use alloc::sync::Arc;
use core::ops::Deref;

use crate::{device::FileDevice, node::Node, ramfs::Directory};

//

extern crate alloc;

pub mod device;
// pub mod error;
pub mod node;
pub mod path;
pub mod ramfs;
// pub mod tree;

//

pub static ROOT: Node = Node::Directory(Ref::from_ref(&Directory::new()));

//

pub type FileRef<'a> = Ref<'a, dyn FileDevice + 'static>;
pub type DirRef<'a> = Ref<'a, dyn DirectoryDevice + 'static>;

//

#[derive(Debug, Clone, Copy)]
pub enum Ref<'a, T: ?Sized> {
    Arc(Arc<T>),
    Ref(&'a T),
}

impl<'a, T: ?Sized> Ref<'a, T> {
    pub fn new(val: T) -> Self {
        Ref::Arc(Arc::new(val))
    }

    pub const fn from_arc(val: Arc<T>) -> Self {
        Ref::Arc(val)
    }

    pub const fn from_ref(val: &'a T) -> Self {
        Ref::Ref(val)
    }
}

impl<'a, T: ?Sized> From<&'a T> for Ref<'a, T> {
    fn from(value: &'a T) -> Self {
        Self::from_ref(value)
    }
}

impl<T: ?Sized> From<Arc<T>> for Ref<'static, T> {
    fn from(value: Arc<T>) -> Self {
        Self::from_arc(value)
    }
}

impl<'a, T: ?Sized> Deref for Ref<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Ref::Arc(v) => &v,
            Ref::Ref(v) => *v,
        }
    }
}

//

#[cfg(test)]
mod tests {
    fn search() {}
}
