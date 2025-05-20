use crate::{
    error::{EcsResult, missing_component},
    id::Id,
    pointer::{Ptr, PtrMut},
    types::type_info::TypeInfo,
};
use std::{rc::Rc, usize};

use super::column::Column;

pub struct SparseData {
    ids: Vec<Id>,
    dense: Column,
    sparse: Vec<usize>,
}

impl SparseData {
    pub fn new(id: Id, type_info: Rc<TypeInfo>) -> Self {
        Self {
            ids: vec![],
            dense: Column::new(id, type_info),
            sparse: vec![],
        }
    }

    /// Resizes the sparse array such that
    /// it can hold at least (`index` + 1) entries.
    #[inline(always)]
    fn ensure_index(&mut self, index: usize) -> usize {
        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, usize::MAX);
        }

        self.sparse[index]
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data if the entity is already in the set.
    ///
    /// # Safety
    /// `val` must match the type of the sparse set items.
    pub unsafe fn insert<C>(&mut self, id: Id, val: C) -> Option<C> {
        let sparse = id.as_usize();
        let dense = self.ensure_index(sparse);

        // SAFETY: Caller ensures that val matches the type of column items.
        if dense < self.dense.len() {
            // SAFETY: We just checked that dense is in bounds
            Some(unsafe { self.dense.set_unchecked(dense, val) })
        } else {
            self.sparse[sparse] = self.dense.len();
            unsafe { self.dense.push_unchecked(val) };
            self.ids.push(id);
            None
        }
    }

    /// Removes an entity from the set.
    /// Returns the value associated with the entity if it was present.
    ///
    /// # Safety
    /// Caller ensures that `C` matches the item type of the column.
    pub fn remove<C>(&mut self, id: Id) -> Option<C> {
        let dense = match self.sparse.get_mut(id.as_usize()) {
            Some(dense) if *dense < self.dense.len() => dense,
            _ => return None, // entity not in set.
        };

        let dense = std::mem::replace(dense, usize::MAX);
        // SAFETY: caller ensures that C matches the item type of the column.
        self.ids.swap_remove(dense);
        let removed = unsafe { self.dense.swap_remove_typed(dense) };

        if dense != self.dense.len() {
            self.sparse[self.ids[dense].as_usize()] = dense;
        }

        Some(removed)
    }

    #[inline]
    pub fn contains(&self, id: Id) -> bool {
        match self.sparse.get(id.as_usize()) {
            Some(&dense) => dense < self.dense.len(),
            None => false,
        }
    }

    #[inline]
    pub fn get_ptr(&self, id: Id) -> EcsResult<Ptr> {
        match self.sparse.get(id.as_usize()) {
            Some(&dense) if dense < self.dense.len() => {
                // SAFETY: We just checked dense is in bounds.
                Ok(unsafe { self.dense.get_ptr(dense) })
            }
            _ => Err(missing_component(id, self.dense.id())),
        }
    }

    #[inline]
    pub fn get_ptr_mut(&mut self, id: Id) -> EcsResult<PtrMut> {
        match self.sparse.get(id.as_usize()) {
            Some(&dense) if dense < self.dense.len() => {
                // SAFETY: We just checked dense is in bounds.
                Ok(unsafe { self.dense.get_ptr_mut(dense) })
            }
            _ => Err(missing_component(id, self.dense.id())),
        }
    }
}

pub struct SparseTag {
    ids: Vec<Id>,
    sparse: Vec<usize>,
}

impl SparseTag {
    pub fn new() -> Self {
        Self {
            ids: vec![],
            sparse: vec![],
        }
    }

    /// Resizes the sparse array such that
    /// it can hold at least (`index` + 1) entries.
    #[inline(always)]
    fn ensure_index(&mut self, index: usize) -> usize {
        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, usize::MAX);
        }
        self.sparse[index]
    }

    /// Inserts a the id into the sparse set.
    pub fn insert(&mut self, id: Id) {
        let sparse = id.as_usize();
        let dense = self.ensure_index(sparse);

        if dense > self.ids.len() {
            self.sparse[sparse] = self.ids.len();
            self.ids.push(id);
        }
    }

    /// Removes an entity from the set.
    /// Returns the value associated with the entity if it was present.
    ///
    /// # Safety
    /// Caller ensures that `C` matches the item type of the column.
    pub fn remove(&mut self, id: Id) {
        let dense = match self.sparse.get_mut(id.as_usize()) {
            Some(dense) if *dense < self.ids.len() => dense,
            _ => return, // entity not in set.
        };

        let dense = std::mem::replace(dense, usize::MAX);
        let _ = self.ids.swap_remove(dense);

        if dense != self.ids.len() {
            self.sparse[self.ids[dense].as_usize()] = dense;
        }
    }

    #[inline]
    pub fn contains(&self, id: Id) -> bool {
        match self.sparse.get(id.as_usize()) {
            Some(&dense) => dense < self.ids.len(),
            None => false,
        }
    }
}

pub trait SparseIndex: PartialEq + Clone + Eq {
    fn to_sparse_index(&self) -> usize;
}

impl SparseIndex for usize {
    #[inline(always)]
    fn to_sparse_index(&self) -> usize {
        *self
    }
}

pub(crate) struct Entry<K: SparseIndex, V = K> {
    key: K,
    pub(crate) value: V,
}

impl<K: SparseIndex, V> Entry<K, V> {
    #[inline]
    pub fn key(&self) -> &K {
        &self.key
    }
}

pub struct SparseSet<K: SparseIndex + PartialEq, V> {
    dense: Vec<Entry<K, V>>,
    sparse: Vec<usize>,
}

impl<K: SparseIndex + PartialEq, V> SparseSet<K, V> {
    pub fn new() -> Self {
        Self {
            dense: vec![],
            sparse: vec![],
        }
    }

    /// Resizes the sparse array such that
    /// it can hold at least (`index` + 1) entries.
    #[inline(always)]
    fn ensure_index(&mut self, index: usize) -> usize {
        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, usize::MAX);
        }

        self.sparse[index]
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data if the entity is already in the set.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let sparse = key.to_sparse_index();
        let dense = self.ensure_index(sparse);

        if dense < self.dense.len() {
            let entry = &mut self.dense[dense];
            if entry.key == key {
                Some(std::mem::replace(&mut entry.value, value))
            } else {
                None
            }
        } else {
            self.sparse[sparse] = self.dense.len();
            self.dense.push(Entry { key, value });
            None
        }
    }

    /// Removes an entity from the set.
    /// Returns the value associated with the entity if it was present.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let sparse = key.to_sparse_index();
        let dense = match self.sparse.get_mut(sparse) {
            Some(dense) if *dense < self.dense.len() && &self.dense[*dense].key == key => dense,
            _ => return None, // key not in set
        };

        let dense = std::mem::replace(dense, usize::MAX);
        let removed = self.dense.swap_remove(dense).value;

        if dense != self.dense.len() {
            self.sparse[self.dense[dense].key.to_sparse_index()] = dense;
        }

        Some(removed)
    }

    #[inline]
    pub fn contains_key(&self, key: &K) -> bool {
        match self.sparse.get(key.to_sparse_index()) {
            Some(&dense) => dense < self.dense.len(),
            None => false,
        }
    }

    #[inline]
    pub fn get(&self, key: &K) -> Option<&V> {
        match self.sparse.get(key.to_sparse_index()) {
            Some(&dense) if dense < self.dense.len() => Some(&self.dense[dense].value),
            _ => None,
        }
    }

    #[inline]
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        match self.sparse.get(key.to_sparse_index()) {
            Some(&dense) if dense < self.dense.len() => Some(&mut self.dense[dense].value),
            _ => None,
        }
    }
}
