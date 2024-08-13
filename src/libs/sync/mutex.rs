use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

pub struct Mutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    #[inline]
    pub const fn new(data: T) -> Self {
        return Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        };
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        while self
            .locked
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            // spin lock
            core::hint::spin_loop()
        }
        return MutexGuard { mutex: self };
    }
}

impl<T> core::fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let locked = self.locked.load(Ordering::SeqCst);
        write!(f, "Mutex: {{ data: ")?;

        if locked {
            write!(f, "<locked> }}")
        } else {
            write!(f, "{:?} }}", self.data)
        }
    }
}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::SeqCst);
    }
}
