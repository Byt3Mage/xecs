use super::column::Column;
use crate::{
    component::Component,
    id::Id,
    pointer::{Ptr, PtrMut},
};

pub(crate) struct TableData {
    pub(super) entities: Vec<Id>,
    pub(super) columns: Box<[Column]>,
}

impl TableData {
    pub(crate) fn new(columns: Box<[Column]>) -> Self {
        Self {
            entities: vec![],
            columns,
        }
    }

    /// Returns number of rows in this table.
    #[inline]
    pub(crate) fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Creates a new row without initializing its elements.
    /// This function will grow all columns if necessary.
    ///
    /// # Safety
    /// - The rows for the new entity in all columns will be uninitialized (hence, unsafe).
    /// - The caller must ensure to write to all the columns in the new row.
    pub(crate) unsafe fn new_row(&mut self, entity: Id) -> usize {
        let row = self.entities.len();
        self.entities.reserve(1);
        self.entities.push(entity);
        row
    }

    // TODO: docs
    pub(crate) unsafe fn push<C: Component>(&mut self, col: usize, val: C) {
        debug_assert!(col < self.columns.len(), "column out of bounds");
        unsafe { self.columns.get_unchecked_mut(col).push(val) }
    }

    /// Returns a reference to the element at `row`, in `column`.
    ///
    /// This function does not perform bounds checking.
    ///
    /// # Safety
    /// - Caller ensures that `row` and `column` are valid.
    pub(crate) unsafe fn get_ptr(&self, col: usize, row: usize) -> Ptr {
        debug_assert!(col < self.columns.len(), "column out of bounds");
        // SAFETY: The caller ensures that `row` and `column` in bounds.
        unsafe { self.columns.get_unchecked(col).get_ptr(row) }
    }

    /// Returns a mutable reference to the element at `row`, in `column`.
    ///
    /// This function does not perform bounds checking.
    ///
    /// # Safety
    /// - The caller ensures that `row` and `column` are valid.
    pub(crate) unsafe fn get_ptr_mut(&mut self, col: usize, row: usize) -> PtrMut {
        debug_assert!(col < self.columns.len(), "TableData: column out of bounds");
        // SAFETY: The caller ensures that `row` and `column` in bounds.
        unsafe { self.columns.get_unchecked_mut(col).get_ptr_mut(row) }
    }

    /// Returns the entity at the specified `row`.
    ///
    /// # Safety
    /// - The row must be valid.
    pub(crate) unsafe fn get_entity_unchecked(&self, row: usize) -> &Id {
        debug_assert!(row < self.entities.len(), "TableData: row out of bounds");
        // SAFETY: The caller ensures that `row` is valid.
        unsafe { self.entities.get_unchecked(row) }
    }

    /// # Safety
    /// - `row` must be in bounds
    /// - `drop_check` must have the same length as `self.columns`
    pub(super) unsafe fn delete_row(&mut self, row: usize, drop_check: &[bool]) -> Option<Id> {
        debug_assert!(row < self.entities.len(), "TableData: row out of bounds");
        debug_assert!(drop_check.len() == self.columns.len());

        for (col, &should_drop) in self.columns.iter_mut().zip(drop_check) {
            unsafe {
                if should_drop {
                    col.swap_remove_drop(row);
                } else {
                    col.swap_remove(row);
                }
            }
        }

        let removed = self.entities.swap_remove(row);

        if row == self.entities.len() {
            None
        } else {
            Some(removed)
        }
    }
}
