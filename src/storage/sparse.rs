use crate::{
    component::Component,
    data_structures::{ErasedVec, SparseIndex},
    id::Entity,
    type_info::TypeInfo,
    type_traits::Data,
};
use std::{rc::Rc, usize};

struct Entry<T: Component<DataType = Data>> {
    entity: Entity,
    val: T,
}

pub(crate) struct SparseData {
    dense: ErasedVec,
    sparse: Vec<usize>,
}

impl SparseData {
    pub(crate) fn new(type_info: Rc<TypeInfo>) -> Self {
        Self {
            dense: ErasedVec::new(type_info),
            sparse: vec![],
        }
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data if the entity is already in the set.
    ///
    /// # Safety
    /// Caller ensures `T` is the same type as the set items.
    pub(crate) unsafe fn insert<T>(&mut self, id: Entity, val: T) -> Option<T>
    where
        T: Component<DataType = Data>,
    {
        let sparse = id.idx();

        if sparse >= self.sparse.len() {
            self.sparse.resize(sparse + 1, usize::MAX);
        }

        let dense = &mut self.sparse[sparse];

        // SAFETY: Caller ensures that T matches the type of ErasedVec items.
        unsafe {
            if *dense < self.dense.len() {
                // SAFETY: We just checked that dense is in bounds
                Some(self.dense.replace(*dense, Entry { entity: id, val }).val)
            } else {
                *dense = self.dense.len();
                self.dense.push(Entry { entity: id, val });
                None
            }
        }
    }

    /// Removes an entity from the set and returns its Data if present.
    ///
    /// # Safety
    /// Caller ensures `T` is the same type as the set items.
    #[inline(always)]
    pub(crate) unsafe fn remove<T>(&mut self, e: Entity) -> Option<T>
    where
        T: Component<DataType = Data>,
    {
        let dense = match self.sparse.get_mut(e.idx()) {
            Some(dense) if *dense < self.dense.len() => dense,
            _ => return None, // id not in set.
        };

        let dense = std::mem::replace(dense, usize::MAX);

        // SAFETY:
        // - Caller ensures T matches the item type of the set
        // - Dense index is valid for the ErasedVec.
        unsafe {
            let removed = self.dense.swap_remove::<Entry<T>>(dense);

            if dense < self.dense.len() {
                let e = self.dense.get::<Entry<T>>(dense).entity;
                self.sparse[e.idx()] = dense;
            }

            Some(removed.val)
        }
    }

    #[inline(always)]
    pub(crate) fn contains(&self, e: Entity) -> bool {
        self.sparse
            .get(e.idx())
            .is_some_and(|&d| d < self.dense.len())
    }

    #[inline]
    pub(crate) unsafe fn get<T>(&self, e: Entity) -> Option<&T>
    where
        T: Component<DataType = Data>,
    {
        let dense = *self.sparse.get(e.idx())?;

        if dense >= self.dense.len() {
            return None;
        }

        // SAFETY:
        // - We just checked dense is in bounds.
        // - Caller ensures T is dense item type
        let entry = unsafe { self.dense.get::<Entry<T>>(dense) };

        Some(&entry.val)
    }

    #[inline]
    pub(crate) unsafe fn get_mut<T>(&mut self, e: Entity) -> Option<&mut T>
    where
        T: Component<DataType = Data>,
    {
        let dense = *self.sparse.get(e.idx())?;

        if dense >= self.dense.len() {
            return None;
        }

        // SAFETY:
        // - We just checked dense is in bounds.
        // - Caller ensures T is dense item type
        let entry = unsafe { self.dense.get_mut::<Entry<T>>(dense) };

        Some(&mut entry.val)
    }
}

pub(crate) struct SparseTag {
    ids: Vec<Entity>,
    sparse: Vec<usize>,
}

impl SparseTag {
    pub(crate) fn new() -> Self {
        Self {
            ids: vec![],
            sparse: vec![],
        }
    }

    /// Inserts a the id into the sparse set.
    pub(crate) fn insert(&mut self, e: Entity) {
        let sparse = e.idx();

        if sparse >= self.sparse.len() {
            self.sparse.resize(sparse + 1, usize::MAX);
        }

        // SAFETY: we just ensured capacity for sparse index.
        let dense = unsafe { self.sparse.get_unchecked_mut(sparse) };
        let len = self.ids.len();

        if *dense > len {
            *dense = len;
            self.ids.push(e);
        }
    }

    /// Removes an entity from the set.
    pub(crate) fn remove(&mut self, e: Entity) {
        let dense = match self.sparse.get_mut(e.idx()) {
            Some(dense) if *dense < self.ids.len() => dense,
            _ => return, // entity not in set.
        };

        let dense = std::mem::replace(dense, usize::MAX);
        self.ids.swap_remove(dense);

        if dense != self.ids.len() {
            self.sparse[self.ids[dense].idx()] = dense;
        }
    }

    #[inline]
    pub fn contains(&self, e: Entity) -> bool {
        match self.sparse.get(e.idx()) {
            Some(&dense) => dense < self.ids.len(),
            None => false,
        }
    }
}
