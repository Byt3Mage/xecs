use std::sync::atomic::{AtomicIsize, Ordering};

#[derive(Debug)]
pub struct AtomicBorrow {
    // > 0 = N shared borrows, -1 = exclusive borrow, 0 = free
    state: AtomicIsize,
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq, Hash)]
#[error("Already borrowed exclusively; cannot share")]
pub struct BorrowRefError;

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq, Hash)]
#[error("Already borrowed; cannot take exclusive")]
pub struct BorrowMutError;

// This ensures the panicking code is outlined from `borrow` for `AtomicBorrow`.
#[cfg_attr(not(panic = "immediate-abort"), inline(never))]
#[track_caller]
#[cold]
fn panic_borrow_mut(err: BorrowMutError) -> ! {
    panic!("{err}")
}

// This ensures the panicking code is outlined from `borrow` for `AtomicBorrow`.
#[cfg_attr(not(panic = "immediate-abort"), inline(never))]
#[track_caller]
#[cold]
fn panic_borrow_ref(err: BorrowRefError) -> ! {
    panic!("{err}")
}

impl AtomicBorrow {
    pub const fn new() -> Self {
        Self { state: AtomicIsize::new(0) }
    }

    /// Immutably borrows the column.
    ///
    /// The borrow lasts until the returned `Ref` exits scope. Multiple
    /// immutable borrows can be taken out at the same time.
    ///
    /// # Panics
    ///
    /// Panics if the column is currently mutably borrowed. For a non-panicking variant, use
    /// [`try_borrow`](Self::try_borrow).
    pub fn borrow(&self) -> BorrowRef<'_> {
        match self.try_borrow() {
            Ok(b) => b,
            Err(e) => panic_borrow_ref(e),
        }
    }

    /// Immutably borrows the column,
    /// returning an error if the value is currently mutably borrowed.
    ///
    /// The borrow lasts until the returned `Ref` exits scope. Multiple immutable borrows can be
    /// taken out at the same time.
    ///
    /// This is the non-panicking variant of [`borrow`](Self::borrow).
    pub fn try_borrow(&self) -> Result<BorrowRef<'_>, BorrowRefError> {
        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            if s < 0 {
                return Err(BorrowRefError);
            }
            match self
                .state
                .compare_exchange_weak(s, s + 1, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => return Ok(BorrowRef { borrow: self }),
                Err(actual) => s = actual, // reload, retry (spurious or racing reader)
            }
        }
    }

    /// Mutably borrows the column.
    ///
    /// The borrow lasts until the returned `RefMut` or all `RefMut`s derived
    /// from it exit scope. The value cannot be borrowed while this borrow is
    /// active.
    ///
    /// # Panics
    /// Panics if the value is currently borrowed. For a non-panicking variant, use
    /// [`try_borrow_mut`](Self::try_borrow_mut).
    pub fn borrow_mut(&self) -> BorrowMut<'_> {
        match self.try_borrow_mut() {
            Ok(b) => b,
            Err(e) => panic_borrow_mut(e),
        }
    }

    /// Mutably borrows the column,
    /// returning an error if the column is currently borrowed.
    ///
    /// The borrow lasts until the returned `RefMut` or all `RefMut`s derived
    /// from it exit scope. The column cannot be borrowed while this borrow is
    /// active.
    ///
    /// This is the non-panicking variant of [`borrow_mut`](Self::borrow_mut).
    pub fn try_borrow_mut(&self) -> Result<BorrowMut<'_>, BorrowMutError> {
        match self.state.compare_exchange(0, -1, Ordering::Acquire, Ordering::Relaxed) {
            Ok(_) => Ok(BorrowMut { borrow: self }),
            Err(_) => Err(BorrowMutError),
        }
    }
}

/// RAII guard for a shared borrow. Releases on drop.
#[derive(Debug)]
pub struct BorrowRef<'a> {
    borrow: &'a AtomicBorrow,
}

impl Drop for BorrowRef<'_> {
    fn drop(&mut self) {
        self.borrow.state.fetch_sub(1, Ordering::Release);
    }
}

/// RAII guard for an exclusive borrow. Releases on drop.
#[derive(Debug)]
pub struct BorrowMut<'a> {
    borrow: &'a AtomicBorrow,
}

impl Drop for BorrowMut<'_> {
    fn drop(&mut self) {
        self.borrow.state.store(0, Ordering::Release);
    }
}
