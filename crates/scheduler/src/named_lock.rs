pub trait LockNamed<T: ?Sized> {
    fn lock_named(&self, name: &'static str) -> NamedMutexGuard<T>;
}

impl<T: ?Sized> LockNamed<T> for Mutex<T> {
    fn lock_named(&self, name: &'static str) -> NamedMutexGuard<T> {
        debug!("locking {name}");
        NamedMutexGuard {
            inner: self.lock(),
            name,
        }
    }
}

pub struct NamedMutexGuard<'a, T: ?Sized + 'a> {
    inner: MutexGuard<'a, T>,
    name: &'static str,
}

impl<'a, T: ?Sized> core::ops::Deref for NamedMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T: ?Sized> core::ops::DerefMut for NamedMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'a, T: ?Sized> Drop for NamedMutexGuard<'a, T> {
    fn drop(&mut self) {
        debug!("unlocking {}", self.name);
    }
}
