//! Lazy initialized value that doesn't get initialized with int_* calls

#![no_std]

//

use core::{cell::Cell, fmt};

use spin::Once;

//

pub struct IntSafeLazy<T, F = fn() -> T> {
    cell: Once<T>,
    init: Cell<Option<F>>,
}

impl<T: fmt::Debug, F> fmt::Debug for IntSafeLazy<T, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lazy")
            .field("cell", &self.cell)
            .field("init", &"..")
            .finish()
    }
}

// We never create a `&F` from a `&Lazy<T, F>` so it is fine
// to not impl `Sync` for `F`
// we do create a `&mut Option<F>` in `force`, but this is
// properly synchronized, so it only happens once
// so it also does not contribute to this impl.
unsafe impl<T, F: Send> Sync for IntSafeLazy<T, F> where Once<T>: Sync {}
// auto-derived `Send` impl is OK.

impl<T, F> IntSafeLazy<T, F> {
    /// Creates a new lazy value with the given initializing
    /// function.
    pub const fn new(f: F) -> Self {
        Self {
            cell: Once::new(),
            init: Cell::new(Some(f)),
        }
    }
    /// Retrieves a mutable pointer to the inner data.
    ///
    /// This is especially useful when interfacing with low level code or FFI where the caller
    /// explicitly knows that it has exclusive access to the inner data. Note that reading from
    /// this pointer is UB until initialized or directly written to.
    pub fn as_mut_ptr(&self) -> *mut T {
        self.cell.as_mut_ptr()
    }
}

impl<T, F: FnOnce() -> T> IntSafeLazy<T, F> {
    /// This is not the interrupt safe method for retrieving
    /// the value, this may cause a deadlock if the function
    /// `F` calls back to this.
    ///
    /// Forces the evaluation of this lazy value and
    /// returns a reference to result. This is equivalent
    /// to the `Deref` impl, but is explicit.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin::Lazy;
    ///
    /// let lazy = Lazy::new(|| 92);
    ///
    /// assert_eq!(Lazy::force(&lazy), &92);
    /// assert_eq!(&*lazy, &92);
    /// ```
    pub fn get_force(&self) -> &T {
        self.cell.call_once(|| match self.init.take() {
            Some(f) => f(),
            None => panic!("Lazy instance has previously been poisoned"),
        })
    }

    /// This is the interrupt safe method for retrieving
    /// the value without blocking.
    ///
    /// Does not force the evaluation of this lazy value,
    /// but returns a reference to the result if it was
    /// already evaluated.
    pub fn get(&self) -> Option<&T> {
        self.cell.get()
    }
}

impl<T: Default> Default for IntSafeLazy<T, fn() -> T> {
    /// Creates a new lazy value using `Default` as the initializing function.
    fn default() -> Self {
        Self::new(T::default)
    }
}
