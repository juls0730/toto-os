use core::ops::Deref;

use super::Cell;

#[derive(PartialEq)]
enum LazyState<T, F = fn() -> T> {
    Uninitialized(F),
    Initializing,
    Initialized(T),
}

pub struct LazyCell<T, F = fn() -> T> {
    state: Cell<LazyState<T, F>>,
}

impl<T, F: Fn() -> T> LazyCell<T, F> {
    pub const fn new(init_func: F) -> Self {
        Self {
            state: Cell::new(LazyState::Uninitialized(init_func)),
        }
    }

    pub fn get(&self) -> Option<&T> {
        match self.state.get() {
            LazyState::Initialized(data) => Some(data),
            _ => None,
        }
    }
}

impl<T, F: Fn() -> T> Deref for LazyCell<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self.state.get() {
            LazyState::Uninitialized(func) => {
                self.state.set(LazyState::Initializing);

                // initialize and return value
                let new_value = func();

                self.state.set(LazyState::Initialized(new_value));

                self.get().unwrap()
            }
            LazyState::Initialized(data) => data,
            LazyState::Initializing => {
                panic!("Attempted to initialize Lazy while initializing Lazy!")
            }
        }
    }
}

unsafe impl<T, F: Fn() -> T + Send> Sync for LazyCell<T, F> {}
