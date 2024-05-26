use alloc::sync::{Arc, Weak};
use core::fmt;

use hyperion_log::*;
use lock_api::{Mutex, RawMutex};

use crate::{
    device::DirectoryDevice,
    error::{IoError, IoResult},
    path::Path,
    ramdisk::{Directory, File},
    AnyMutex, FileDevice, Ref,
};

//

pub type Root = DirRef;
