use crate::{id::Key, type_info::TypeInfo};
use std::{
    ptr::{self, NonNull},
    rc::Rc,
};

/// Type-erased vector of component values
///
/// This data structure is designed to be managed by other structs.
pub(crate) struct ColumnVec<K: Key> {
    id: K,
    data: NonNull<u8>,
    len: usize,
    cap: usize,
    type_info: Rc<TypeInfo>,
}

impl<K: Key> ColumnVec<K> {
    pub fn new(id: K, type_info: Rc<TypeInfo>) -> Self {
        Self {
            id,
            data: (type_info.dangling)(),
            len: 0,
            cap: if type_info.size == 0 { usize::MAX } else { 0 },
            type_info,
        }
    }

    #[inline]
    pub(crate) fn id(&self) -> &K {
        &self.id
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn reserve(&mut self, additional: usize) {
        let new_cap = self.len + additional;

        if new_cap <= self.cap {
            return;
        }

        // since we set the capacity to usize::MAX when the type has size 0,
        // getting here means the Vec is overfull.
        assert_ne!(self.type_info.size, 0, "capacity overflow");

        let new_cap = new_cap.next_power_of_two();

        assert_ne!(new_cap, 0);

        let new_layout = (self.type_info.arr_layout)(new_cap).unwrap();

        let ptr = unsafe {
            if self.cap == 0 {
                std::alloc::alloc(new_layout)
            } else {
                let old_layout = (self.type_info.arr_layout)(self.cap).unwrap();
                std::alloc::realloc(self.data.as_ptr(), old_layout, new_layout.size())
            }
        };

        self.data = match NonNull::new(ptr) {
            Some(ptr) => ptr,
            None => std::alloc::handle_alloc_error(new_layout),
        };

        self.cap = new_cap;
    }

    pub(super) unsafe fn push<T>(&mut self, val: T) {
        self.reserve(1);
        unsafe { self.data.as_ptr().cast::<T>().add(self.len).write(val) };
        self.len += 1;
    }

    /// # Safety
    /// - Caller must ensure that `row` is valid for this column.
    /// - Caller must ensure that `T` is the value type of this column.
    #[inline]
    pub(super) unsafe fn get<T>(&self, row: usize) -> &T {
        debug_assert!(row < self.len, "Column: row out of bounds");

        // SAFETY:
        // - self.data is non-null and aligned for T
        // - caller guarantees row is valid.
        unsafe { self.data.cast::<T>().add(row).as_ref() }
    }

    /// # Safety
    /// - Caller must ensure that `row` is valid for this column.
    /// - Caller must ensure that `T` is the value type of this column.
    #[inline]
    pub(super) unsafe fn get_mut<T>(&mut self, row: usize) -> &mut T {
        debug_assert!(row < self.len, "Column: row out of bounds");

        // SAFETY:
        // data is non-null
        // caller guarantees row is valid.
        unsafe { self.data.cast::<T>().add(row).as_mut() }
    }

    /// # Safety
    /// - Caller must ensure that `row` is valid for this column.
    #[inline]
    pub(super) unsafe fn get_ptr(&self, row: usize) -> NonNull<u8> {
        debug_assert!(row < self.len, "Column: row out of bounds");
        // SAFETY:
        // data is non-null
        // caller guarantees row is valid.
        unsafe { self.data.add(row * self.type_info.size) }
    }

    /// # Safety
    /// - Caller must ensure that `row` is valid for this column.
    #[inline]
    pub(super) unsafe fn get_ptr_mut(&mut self, row: usize) -> NonNull<u8> {
        debug_assert!(row < self.len, "Column: row out of bounds");
        // SAFETY:
        // data is non-null
        // caller guarantees row is valid.
        unsafe { self.data.add(row * self.type_info.size) }
    }

    /// Removes this row by swapping with the last row and dropping its value.
    ///
    /// # Panics
    /// if `row` is out of bounds.
    pub(super) fn swap_remove_drop(&mut self, row: usize) {
        assert!(row < self.len, "Column: row out of bounds");

        let size = self.type_info.size;
        let last_row = self.len - 1;

        unsafe {
            let base = self.data.as_ptr();
            let last_ptr = base.add(last_row * size);

            if row != last_row {
                ptr::swap_nonoverlapping(base.add(row * size), last_ptr, size);
            }

            self.len = last_row;

            if let Some(drop_fn) = self.type_info.drop_fn {
                drop_fn(last_ptr)
            }
        }
    }

    /// Removes this row by swapping with the last row. DOES NOT DROP the removed row.
    ///
    /// # Safety
    /// - `row` must be in bounds for this column
    /// - Caller must ensure that the row does not require dropping
    pub(super) unsafe fn swap_remove(&mut self, row: usize) {
        debug_assert!(row < self.len, "Column: row out of bounds");

        let size = self.type_info.size;
        let last_row = self.len - 1;

        if row != last_row {
            unsafe {
                let base = self.data.as_ptr();
                let row_ptr = base.add(row * size);
                let lst_ptr = base.add(last_row * size);

                ptr::swap_nonoverlapping(row_ptr, lst_ptr, size);
            }
        }

        self.len = last_row;
    }

    /// Moves the data from `src_row` and appends to dest [Column].
    /// The data is copied, so callers must ensure not to read from row again.
    ///
    /// # Safety
    /// Caller must ensure that `src_row` is valid in self.
    /// Caller must ensure that `self`, and `dest` hold the same item type.
    pub(super) unsafe fn move_row_to(&mut self, src_row: usize, dest: &mut Self) {
        let size = self.type_info.size;

        // SAFETY:
        // Callers uphold the following guarantees:
        // src_row and dst_row are valid in their columns
        // both columns hold the same item type
        // src_row is never read from again unless overwritten.
        unsafe {
            dest.reserve(1);

            let src_data = self.data.as_ptr().add(src_row * size);
            let dst_data = dest.data.as_ptr().add(dest.len * size);
            ptr::copy_nonoverlapping(src_data, dst_data, size);

            dest.len += 1;
        }
    }
}

impl<K: Key> Drop for ColumnVec<K> {
    fn drop(&mut self) {
        if self.cap == 0 || self.type_info.size == 0 {
            return;
        }

        unsafe {
            let size = self.type_info.size;
            let layout = (self.type_info.arr_layout)(self.cap).unwrap();

            if let Some(drop_fn) = self.type_info.drop_fn {
                let mut ptr = self.data.as_ptr();
                for _ in 0..self.len {
                    drop_fn(ptr);
                    ptr = ptr.add(size)
                }
            }

            std::alloc::dealloc(self.data.as_ptr(), layout);
        }
    }
}
