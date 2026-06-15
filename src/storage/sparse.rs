use std::{mem, rc::Rc};

use crate::{id::Id, storage::column::Column, type_meta::TypeMeta};

#[derive(Debug)]
pub(crate) struct SparseSet {
    ids: Vec<Id>,
    column: Column,
    rows: Vec<usize>,
}

impl Drop for SparseSet {
    fn drop(&mut self) {
        let len = self.row_count();
        unsafe { self.column.destroy(len, len) };
    }
}

impl SparseSet {
    pub(crate) fn new(id: Id, type_meta: Rc<TypeMeta>) -> Self {
        Self {
            column: Column::new(id, type_meta),
            rows: vec![],
            ids: vec![],
        }
    }

    #[inline]
    pub fn row_count(&self) -> usize {
        self.ids.len()
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data if the entity is already in the set.
    ///
    /// # Safety
    /// Caller ensures `T` is the same type as the set items.
    pub(crate) unsafe fn insert<T: 'static>(&mut self, id: Id, val: T) -> Option<T> {
        let row_idx = id.idx() as usize;

        if row_idx >= self.rows.len() {
            self.rows.resize(row_idx + 1, usize::MAX);
        }

        // SAFETY: Caller ensures that T matches the type of column.
        unsafe {
            let row = self.rows[row_idx];
            if row < self.row_count() {
                return Some(self.column.replace(row, val));
            }

            let old_len = self.row_count();
            let new_len = old_len.checked_add(1).unwrap();
            self.column.realloc(old_len, new_len);
            self.ids.reserve(1);

            self.column.write(old_len, val);
            self.ids.push(id);
            self.rows[row_idx] = old_len;

            None
        }
    }

    /// Removes an entity from the set.
    #[inline(always)]
    pub(crate) fn remove(&mut self, id: Id) {
        let row_idx = id.idx() as usize;

        let row = match self.rows.get_mut(row_idx) {
            Some(r) if *r < self.ids.len() => mem::replace(r, usize::MAX),
            _ => return,
        };

        // SAFETY:
        // - Caller ensures T matches the item type of the set
        // - Row is valid for the column.
        unsafe {
            self.column.drop_row(row);

            let last = self.row_count() - 1;

            if row != last {
                self.column.copy_row(last, row);
                self.ids.swap_remove(row);
                self.rows[self.ids[row].idx() as usize] = row;
            } else {
                self.ids.pop();
            }
        }
    }

    #[inline(always)]
    pub(crate) fn contains(&self, id: Id) -> bool {
        self.rows.get(id.idx() as usize).is_some_and(|&r| r < self.row_count())
    }

    #[inline]
    pub(crate) unsafe fn get<T: 'static>(&self, id: Id) -> Option<&T> {
        // SAFETY: We just checked row is in bounds.
        self.rows
            .get(id.idx() as usize)
            .filter(|&&r| r < self.row_count())
            .map(|&r| unsafe { self.column.get(r) })
    }

    #[inline]
    pub(crate) unsafe fn get_mut<T: 'static>(&self, id: Id) -> Option<&mut T> {
        // SAFETY: We just checked row is in bounds.
        self.rows
            .get(id.idx() as usize)
            .filter(|&&r| r < self.row_count())
            .map(|&r| unsafe { self.column.get_mut(r) })
    }
}
