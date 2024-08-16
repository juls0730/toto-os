use core::cell::UnsafeCell;

mod once;

pub use once::OnceCell;

pub struct Cell<T: ?Sized> {
    value: UnsafeCell<T>,
}

impl<T: ?Sized> !Sync for Cell<T> {}

impl<T> Cell<T> {
    pub const fn new(value: T) -> Cell<T> {
        return Self {
            value: UnsafeCell::new(value),
        };
    }

    pub fn get(&self) -> &T {
        return unsafe { &*self.value.get() };
    }

    pub fn get_mut(&self) -> &mut T {
        return unsafe { &mut *self.value.get() };
    }

    pub fn set(&self, new_value: T) {
        unsafe { *self.value.get() = new_value };
    }
}
