use alloc::boxed::Box;
use core::fmt::Display;

use async_trait::async_trait;
use hyperion_syscall::err::Result;

use crate::{FileDevice, Node, OpenFile, Ref};

//

pub fn new_virtual_ro_open_file(data: impl Display + Send + Sync + 'static) -> Ref<dyn OpenFile> {
    struct DisplayOpenFile<T> {
        #[allow(unused)]
        v: T,
    }

    impl<T: Display + Send + Sync + 'static> OpenFile for DisplayOpenFile<T> {}

    Ref::new(DisplayOpenFile { v: data }).unsize()
}

pub fn new_virual_ro_file(data: impl Display + Send + Sync + 'static) -> Ref<Node> {
    struct DisplayFile {
        v: Ref<dyn OpenFile>,
    }

    #[async_trait]
    impl FileDevice for DisplayFile {
        async fn open(&self) -> Result<Ref<dyn OpenFile>> {
            Ok(self.v.clone())
        }
    }

    Node::new_file(DisplayFile {
        v: new_virtual_ro_open_file(data),
    })
    .into()
}

pub fn new_virual_ro_generator_file<
    F: Send + Sync + 'static + Fn() -> T,
    T: Display + Send + Sync + 'static,
>(
    gen: F,
) -> Ref<Node> {
    struct DisplayGenFile<F> {
        gen: F,
    }

    #[async_trait]
    impl<F: Send + Sync + 'static + Fn() -> T, T: Display + Send + Sync + 'static> FileDevice
        for DisplayGenFile<F>
    {
        async fn open(&self) -> Result<Ref<dyn OpenFile>> {
            Ok(new_virtual_ro_open_file((self.gen)()))
        }
    }

    Node::new_file(DisplayGenFile { gen }).into()
}
