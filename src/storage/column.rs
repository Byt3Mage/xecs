use crate::{
    component::Component,
    id::Id,
    pointer::{Ptr, PtrMut},
    type_info::TypeInfo,
};
use std::{
    alloc::Layout,
    ptr::{self, NonNull},
    rc::Rc,
};

/// Type-erased vector of component values
///
/// This data structure is meant to be managed by other structs.
pub(crate) struct Column {
    id: Id,
    type_info: Rc<TypeInfo>,
    data: NonNull<u8>,
    len: usize,
    cap: usize,
}

impl Column {
    pub fn new(id: Id, type_info: Rc<TypeInfo>) -> Self {
        assert!(type_info.size() != 0, "can't create column for ZST");
        Self {
            id,
            type_info,
            data: NonNull::dangling(),
            len: 0,
            cap: 0,
        }
    }

    #[inline]
    pub(crate) fn id(&self) -> Id {
        self.id
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_ptr()
    }

    pub(crate) fn reserve(&mut self, additional: usize) {
        let new_cap = self.len + additional;

        if new_cap <= self.cap {
            return;
        }

        let new_cap = new_cap.next_power_of_two();

        assert_ne!(new_cap, 0);

        let size = self.type_info.size();
        let align = self.type_info.align();
        let new_layout = Layout::from_size_align(new_cap * size, align).unwrap();

        let ptr = unsafe {
            if self.cap == 0 {
                std::alloc::alloc(new_layout)
            } else {
                let old_layout = Layout::from_size_align(self.cap * size, align).unwrap();
                std::alloc::realloc(self.data.as_ptr(), old_layout, new_layout.size())
            }
        };

        let data = match NonNull::new(ptr) {
            Some(ptr) => ptr,
            None => std::alloc::handle_alloc_error(new_layout),
        };

        self.data = data;
        self.cap = new_cap;
    }

    pub(super) unsafe fn push<C: Component>(&mut self, val: C) {
        self.reserve(1);

        unsafe {
            self.data
                .as_ptr()
                .add(self.len * self.type_info.size())
                .cast::<C>()
                .write(val);
        }

        self.len += 1;
    }

    /// # Safety
    /// - Caller must ensure that `row` is valid for this column.
    #[inline]
    pub(super) unsafe fn get_ptr(&self, row: usize) -> Ptr {
        debug_assert!(row < self.len, "Column: row out of bounds");
        // SAFETY:
        // data is non-null
        // caller guarantees row is valid.
        unsafe { Ptr::new(self.data.add(row * self.type_info.size())) }
    }

    /// # Safety
    /// - Caller must ensure that `row` is valid for this column.
    #[inline]
    pub(super) unsafe fn get_ptr_mut(&mut self, row: usize) -> PtrMut {
        debug_assert!(row < self.len, "Column: row out of bounds");
        // SAFETY:
        // data is non-null
        // caller guarantees row is valid.
        unsafe { PtrMut::new(self.data.add(row * self.type_info.size())) }
    }

    /// Removes this row by swapping with the last row and dropping its value.
    ///
    /// # Panics
    /// Panics if `row` is out of bounds.
    pub(super) fn swap_remove_drop(&mut self, row: usize) {
        assert!(row < self.len, "Column: row out of bounds");

        let size = self.type_info.size();
        let last_row = self.len - 1;

        unsafe {
            let base = self.data.as_ptr();
            let last_ptr = base.add(last_row * size);

            if row != last_row {
                std::ptr::swap_nonoverlapping(base.add(row * size), last_ptr, size);
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

        let size = self.type_info.size();
        let last_row = self.len - 1;

        if row != last_row {
            unsafe {
                let base = self.data.as_ptr();
                let row_ptr = base.add(row * size);
                let lst_ptr = base.add(last_row * size);

                std::ptr::swap_nonoverlapping(row_ptr, lst_ptr, size);
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
        let size = self.type_info.size();

        // SAFETY:
        // Callers uphold the following guarantees:
        // src_row and dst_row are valid in their columns
        // both columns hold the same item type
        // src_row is never read from again unless overwritten.
        unsafe {
            dest.reserve(1);

            let src_data = self.as_mut_ptr().add(src_row * size);
            let dst_data = dest.as_mut_ptr().add(dest.len * size);
            ptr::copy_nonoverlapping(src_data, dst_data, size);

            dest.len += 1;
        }
    }
}

impl Drop for Column {
    fn drop(&mut self) {
        if self.cap == 0 {
            return;
        }

        unsafe {
            let size = self.type_info.size();
            let align = self.type_info.align();
            let layout = Layout::from_size_align(size * self.cap, align).unwrap();

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
