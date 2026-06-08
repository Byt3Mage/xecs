use crate::{
    Ecs,
    id::Id,
    storage::{
        BorrowMutError, BorrowRefError,
        singleton::{SingletonMut, SingletonRef},
        table::{ColumnMut, ColumnRef, Table},
    },
};

pub struct TableIter<'a> {
    pub(crate) ecs: &'a Ecs,
    pub(crate) table: &'a Table,
    pub(crate) col_indices: &'a [usize],
    pub(crate) singletons: &'a [Id],
}

impl<'a> TableIter<'a> {
    #[inline(always)]
    pub fn num_rows(&self) -> usize {
        self.table.num_rows()
    }

    /// The entity ids for this table.
    #[inline(always)]
    pub fn ids(&self) -> &'a [Id] {
        self.table.ids()
    }

    /// Get a read-only slice for field at `index`.
    ///
    /// # Panics
    /// Panics if the column is currently mutably borrowed. For a non-panicking variant, use
    /// [`try_field`](Self::try_field).
    #[inline]
    pub fn field<T: 'static>(&self, idx: usize) -> ColumnRef<'_, T> {
        self.table.column_ref(self.col_indices[idx])
    }

    /// Get a mutable slice for field at `index`.
    ///
    /// # Panics
    /// Panics if the column is currently borrowed. For a non-panicking variant,
    /// use [`try_field_mut`](Self::try_field_mut).
    #[inline]
    pub fn field_mut<T: 'static>(&self, idx: usize) -> ColumnMut<'_, T> {
        self.table.column_mut(self.col_indices[idx])
    }

    /// Get a read-only slice for field at `index`.
    #[inline(always)]
    pub fn try_field<T: 'static>(&self, idx: usize) -> Result<ColumnRef<'_, T>, BorrowRefError> {
        self.table.try_column_ref(self.col_indices[idx])
    }

    /// Get a mutable slice for field at `index`.
    #[inline(always)]
    pub fn try_field_mut<T: 'static>(&self, idx: usize) -> Result<ColumnMut<'_, T>, BorrowMutError> {
        self.table.try_column_mut(self.col_indices[idx])
    }

    pub fn singleton<T: 'static>(&self, idx: usize) -> SingletonRef<'_, T> {
        let cm = &self.ecs.components[self.singletons[idx]];
        cm.singleton.as_ref().unwrap().borrow()
    }

    pub fn singleton_mut<T: 'static>(&self, idx: usize) -> SingletonMut<'_, T> {
        let cm = &self.ecs.components[self.singletons[idx]];
        cm.singleton.as_ref().unwrap().borrow_mut()
    }

    pub fn try_singleton<T: 'static>(&self, idx: usize) -> Result<SingletonRef<'_, T>, BorrowRefError> {
        let cm = &self.ecs.components[self.singletons[idx]];
        cm.singleton.as_ref().unwrap().try_borrow()
    }

    pub fn try_singleton_mut<T: 'static>(&self, idx: usize) -> Result<SingletonMut<'_, T>, BorrowMutError> {
        let cm = &self.ecs.components[self.singletons[idx]];
        cm.singleton.as_ref().unwrap().try_borrow_mut()
    }
}
