//! Atomic (Atomic Ref Counted), `Arc` with atomic swap support

use alloc::{alloc::alloc, boxed::{Box, ThinBox, }};
use core::{alloc::Layout, marker::PhantomData, ops::Deref, ptr::{self, NonNull, Pointee, Thin}, sync::atomic::{AtomicPtr, AtomicUsize}};

//

pub struct Aarc<T: ?Sized> {
    ptr: AtomicPtr<u8>,
    _p: PhantomData<T>,
}

impl<T> Aarc<T> {
    pub fn new(val: T) -> Self {

        let meta = ptr::metadata(&val);
        let val;

        let layout_ref_count = Layout::new::<AtomicUsize>();
        let layout_header = Layout::new::<<T as Pointee>::Metadata>();
        let layout_value = Layout::new::<T>();

        let (layout_meta, header_offset) = layout_ref_count.extend(layout_header).unwrap();
        let (layout, value_offset) = layout_meta.extend(layout_value).unwrap();


        unsafe {
            let ptr = alloc(layout);

            ptr::write(ptr, )
        }



        let layout = Layout::for_value(&val);

        
        
        let inner = Box::leak(Box::new(AarcInner {
            ref_count: AtomicUsize::new(1),
            data: val,
        }));


        let ptr: *mut [u8] = ptr::slice_from_raw_parts_mut(ptr::null_mut(), 1000);
        let (ptr, meta) = ptr.to_raw_parts();

        


        Self::from_inner(inner.into())
    }

    fn from_inner(ptr: NonNull<AarcInner<T>>) -> Self {
        Self { ptr }
    }
}

impl<T: ?Sized> Aarc<T> {
    pub fn from(v: Box<_>) {
        let x = ThinBox::new("");
        let x = ThinBox::new_unsize("");
    }
}

unsafe impl<T: ?Sized + Sync + Send> Send for Aarc<T> {}
unsafe impl<T: ?Sized + Sync + Send> Sync for Aarc<T> {}

impl<T: ?Sized> Deref for Aarc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.ptr
        todo!()
    }
}

//

struct AarcLayout<T: ?Sized> {ptr: NonNull<u8>, _p: PhantomData<T>}

impl<T: ?Sized> AarcLayout<T> {
     fn from_ptr(ptr: *mut u8) -> Self {
         Self { ptr: NonNull::new(ptr).unwrap(), _p: PhantomData }
     }

    fn ref_count(&self) -> NonNull<AtomicUsize> {
        self.ptr.as_ptr().add()
    }
}

//

#[repr(transparent)]
struct WithHeader(NonNull<u8>);

impl WithHeader {
    pub fn header(&self) -> NonNull<> {}
}
