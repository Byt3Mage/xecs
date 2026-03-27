use std::{ptr::NonNull, rc::Rc};

use crate::type_info::TypeInfo;

pub trait SparseIndex: PartialEq + Clone + Eq {
    fn idx(&self) -> usize;
}

impl SparseIndex for usize {
    #[inline(always)]
    fn idx(&self) -> usize {
        *self
    }
}

pub(crate) struct Entry<K: SparseIndex, V = K> {
    pub(crate) key: K,
    pub(crate) value: V,
}

pub struct SparseSet<K: SparseIndex + PartialEq, V> {
    dense: Vec<Entry<K, V>>,
    sparse: Vec<usize>,
}

impl<K: SparseIndex + PartialEq, V> SparseSet<K, V> {
    const INVALID_DENSE_IDX: usize = usize::MAX;

    pub fn new() -> Self {
        Self {
            dense: vec![],
            sparse: vec![],
        }
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data and returns the old value if the entry is already in the set.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let sparse_idx = key.idx();

        if sparse_idx >= self.sparse.len() {
            self.sparse.resize(sparse_idx + 1, Self::INVALID_DENSE_IDX);
        }

        let dense_idx = &mut self.sparse[sparse_idx];

        match self.dense.get_mut(*dense_idx) {
            Some(entry) => Some(std::mem::replace(&mut entry.value, value)),
            None => {
                *dense_idx = self.dense.len();
                self.dense.push(Entry { key, value });
                None
            }
        }
    }

    /// Removes an entry from the set.
    /// Returns the value associated with the key if it was present.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let sparse = key.idx();
        let dense_idx = self.sparse.get_mut(sparse)?;

        if *dense_idx >= self.dense.len() {
            return None;
        }

        let dense_idx = std::mem::replace(dense_idx, Self::INVALID_DENSE_IDX);
        let removed = self.dense.swap_remove(dense_idx).value;

        if let Some(entry) = self.dense.get(dense_idx) {
            self.sparse[entry.key.idx()] = dense_idx;
        }

        Some(removed)
    }

    #[inline]
    pub fn contains_key(&self, key: &K) -> bool {
        self.sparse
            .get(key.idx())
            .is_some_and(|&dense_idx| dense_idx < self.dense.len())
    }

    #[inline]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.sparse
            .get(key.idx())
            .and_then(|&dense_idx| self.dense.get(dense_idx))
            .map(|e| &e.value)
    }

    #[inline]
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.sparse
            .get(key.idx())
            .and_then(|&dense_idx| self.dense.get_mut(dense_idx))
            .map(|e| &mut e.value)
    }
}

pub(crate) struct ErasedVec {
    data: NonNull<u8>,
    len: usize,
    cap: usize,
    type_info: Rc<TypeInfo>,
}

impl ErasedVec {
    pub fn new(type_info: Rc<TypeInfo>) -> Self {
        Self {
            data: (type_info.dangling)(),
            len: 0,
            cap: if type_info.size == 0 { usize::MAX } else { 0 },
            type_info,
        }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn reserve_exact(&mut self, additional: usize) {
        let required_cap = self.len.checked_add(additional).expect("capacity overflow");

        if required_cap <= self.cap {
            return;
        }

        assert_ne!(self.type_info.size, 0, "capacity overflow");

        self.realloc(required_cap);
    }

    #[cold]
    fn grow(&mut self) {
        let new_cap = if self.cap == 0 { 4 } else { self.cap * 2 };
        self.realloc(new_cap);
    }

    #[cold]
    fn realloc(&mut self, new_cap: usize) {
        debug_assert!(new_cap > self.cap);
        debug_assert_ne!(self.type_info.size, 0);

        let new_layout = (self.type_info.arr_layout)(new_cap).expect("allocation layout failed");

        let ptr = unsafe {
            if self.cap == 0 {
                std::alloc::alloc(new_layout)
            } else {
                let old_layout = (self.type_info.arr_layout)(self.cap).expect("layout failed");
                std::alloc::realloc(self.data.as_ptr(), old_layout, new_layout.size())
            }
        };

        self.data = match NonNull::new(ptr) {
            Some(ptr) => ptr,
            None => std::alloc::handle_alloc_error(new_layout),
        };

        self.cap = new_cap;
    }

    /// # Safety
    /// Caller must ensure that `T` is the value type of this column.
    ///
    /// # Panics
    /// Panics if the capacity exceeds `isize::MAX` bytes.
    #[inline(always)]
    pub(super) unsafe fn push<T>(&mut self, val: T) {
        if self.len == self.cap {
            self.grow();
        }

        unsafe { self.data.cast().add(self.len).write(val) };
        self.len += 1;
    }

    #[inline(always)]
    pub(super) unsafe fn replace<T>(&mut self, row: usize, val: T) -> T {
        debug_assert!(row < self.len, "Column: row out of bounds");
        unsafe { self.data.cast().add(row).replace(val) }
    }

    /// # Safety
    /// - Caller must ensure that `row` is valid for this column.
    /// - Caller must ensure that `T` is the value type of this column.
    #[inline(always)]
    pub(super) unsafe fn get<T>(&self, row: usize) -> &T {
        debug_assert!(row < self.len, "Column: row out of bounds");

        // SAFETY:
        // - self.data is non-null and aligned for T
        // - caller guarantees row is valid.
        unsafe { self.data.cast().add(row).as_ref() }
    }

    /// # Safety
    /// - Caller must ensure that `row` is valid for this column.
    /// - Caller must ensure that `T` is the value type of this column.
    #[inline(always)]
    pub(super) unsafe fn get_mut<T>(&mut self, row: usize) -> &mut T {
        debug_assert!(row < self.len, "Column: row out of bounds");

        // SAFETY:
        // data is non-null
        // caller guarantees row is valid.
        unsafe { self.data.cast().add(row).as_mut() }
    }

    /// Removes this row by swapping with the last row.
    ///
    /// # Safety
    /// - Caller must ensure that `row` is valid for this column.
    /// - Caller must ensure that `T` is the value type of this column.
    pub(super) unsafe fn swap_remove<T>(&mut self, row: usize) -> T {
        debug_assert!(row < self.len, "Column: row out of bounds");

        unsafe {
            let row_ptr = self.data.cast().add(row);
            let removed = row_ptr.read();
            let last_row = self.len - 1;

            if row != last_row {
                let last_ptr = self.data.cast().add(last_row);
                row_ptr.copy_from_nonoverlapping(last_ptr, 1);
            }

            self.len = last_row;

            removed
        }
    }
}

impl Drop for ErasedVec {
    fn drop(&mut self) {
        let size = self.type_info.size;

        if size == 0 || self.cap == 0 {
            return;
        }

        unsafe {
            if let Some(drop_fn) = self.type_info.drop_fn {
                let mut ptr = self.data;
                for _ in 0..self.len {
                    drop_fn(ptr);
                    ptr = ptr.add(size)
                }
            }

            let layout = (self.type_info.arr_layout)(self.cap).unwrap();
            std::alloc::dealloc(self.data.as_ptr(), layout);
        }
    }
}
