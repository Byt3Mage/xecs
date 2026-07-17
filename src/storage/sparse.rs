use std::rc::Rc;

use crate::{id::Id, storage::blob::Blob, type_meta::TypeMeta};

#[derive(Debug)]
pub(crate) struct SparseSet {
    ids: Vec<Id>,
    data: Blob,
    rows: Vec<u32>,
}

impl Drop for SparseSet {
    fn drop(&mut self) {
        let len = self.len() as u32;
        unsafe { self.data.destroy(len, len) };
    }
}

impl SparseSet {
    pub(crate) fn new(type_meta: Rc<TypeMeta>) -> Self {
        Self {
            data: Blob::new(type_meta),
            rows: vec![],
            ids: vec![],
        }
    }

    #[inline]
    pub fn len(&self) -> u32 {
        self.ids.len() as u32
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data if the entity is already in the set.
    ///
    /// # Safety
    /// Caller ensures `T` is the same type as the set items.
    pub(crate) unsafe fn insert<T: 'static>(&mut self, id: Id, val: T) -> Option<T> {
        let row_idx = id.index() as usize;

        if row_idx >= self.rows.len() {
            self.rows.resize(row_idx + 1, u32::MAX);
        }

        // SAFETY: Caller ensures that T matches the type of column.
        unsafe {
            let row = self.rows[row_idx];
            if row < self.len() {
                return Some(self.data.replace(row, val));
            }

            let old_len = self.len();
            let new_len = old_len.checked_add(1).unwrap();
            self.data.realloc(old_len as usize, new_len as usize);
            self.ids.reserve(1);

            self.data.write(old_len, val);
            self.ids.push(id);
            self.rows[row_idx] = old_len;

            None
        }
    }

    #[inline(always)]
    pub(crate) fn contains(&self, id: Id) -> bool {
        self.rows.get(id.index() as usize).is_some_and(|&r| r < self.len())
    }

    #[inline]
    pub(crate) unsafe fn get<T: 'static>(&self, id: Id) -> Option<&T> {
        let row = *self.rows.get(id.index() as usize)?;
        // SAFETY: We just checked row is in bounds.
        (row < self.len()).then(|| unsafe { self.data.get(row) })
    }

    #[inline]
    pub(crate) unsafe fn get_mut<T: 'static>(&self, id: Id) -> Option<&mut T> {
        let row = *self.rows.get(id.index() as usize)?;
        // SAFETY: We just checked row is in bounds.
        (row < self.len()).then(|| unsafe { self.data.get_mut(row) })
    }
}
