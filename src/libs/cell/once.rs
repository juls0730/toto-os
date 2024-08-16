use core::ops::Deref;

use super::Cell;

pub struct OnceCell<T> {
    state: Cell<OnceCellState<T>>,
}

unsafe impl<T> Sync for OnceCell<T> {}

enum OnceCellState<T> {
    Uninitialized,
    Initializing,
    Initialized(T),
}

impl<T> OnceCell<T> {
    pub const fn new() -> Self {
        return OnceCell {
            state: Cell::new(OnceCellState::Uninitialized),
        };
    }

    pub fn set(&self, new_data: T) {
        match self.state.get() {
            OnceCellState::Uninitialized => {
                self.state.set(OnceCellState::Initializing);

                self.state.set(OnceCellState::Initialized(new_data));
            }
            _ => panic!("Tried to initialize data that is alread initialized"),
        }
    }

    pub fn get_or_set<F>(&self, func: F) -> &T
    where
        F: FnOnce() -> T,
    {
        match self.state.get() {
            OnceCellState::Uninitialized => {
                self.set(func());
                self.get_unchecked()
            }
            OnceCellState::Initializing => panic!("Tried to get or set data that is initializing"),
            OnceCellState::Initialized(data) => data,
        }
    }

    fn get_unchecked(&self) -> &T {
        match self.state.get() {
            OnceCellState::Initialized(data) => data,
            _ => panic!("Attempted to access uninitialized data!"),
        }
    }

    pub fn get(&self) -> Result<&T, ()> {
        match self.state.get() {
            OnceCellState::Initialized(data) => return Ok(data),
            _ => return Err(()),
        }
    }
}

impl<T> Deref for OnceCell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get_unchecked()
    }
}
