use super::{erased_vec::ErasedVec, sparse_set::SparseIndex};
use crate::{component::ComponentValue, entity::Entity, types::type_info::TypeInfo};
use std::rc::Rc;

/// Type-erased sparse set for storing component data.
pub(crate) struct ComponentSparseSet {
    data: ErasedVec,
    dense: Vec<Entity>,
    sparse: Vec<usize>,
}

impl ComponentSparseSet {
    pub(crate) fn new(type_info: Rc<TypeInfo>) -> Self {
        Self {
            data: ErasedVec::new(type_info),
            dense: vec![],
            sparse: vec![],
        }
    }

    #[inline(always)]
    fn ensure_index(&mut self, index: usize) -> usize {
        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, usize::MAX);
        }

        self.sparse[index]
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data if the entity is already in the set.
    pub(crate) fn insert<C: ComponentValue>(&mut self, entity: Entity, value: C) {
        let sparse = entity.to_sparse_index();
        let dense = self.ensure_index(sparse);

        if dense < self.data.len() {
            self.dense[dense] = entity;
            self.data.set(dense, value);
        } else {
            self.sparse[sparse] = self.data.len();
            self.data.push(value);
            self.dense.push(entity);
        }
    }

    /// Removes an entity from the set.
    /// Returns the value associated with the entity if it was present.
    pub(crate) fn remove<C: ComponentValue>(&mut self, entity: Entity) -> Option<(Entity, C)> {
        let sparse = entity.to_sparse_index();
        match self.sparse.get(sparse) {
            Some(&dense) if dense < self.data.len() => {
                self.sparse[sparse] = usize::MAX;

                let entity = self.dense.swap_remove(dense);
                let value = self.data.swap_remove(dense);

                if dense != self.dense.len() {
                    self.sparse[self.dense[dense].to_sparse_index()] = dense;
                }

                Some((entity, value))
            }
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn has(&self, entity: Entity) -> bool {
        match self.sparse.get(entity.to_sparse_index()) {
            Some(&dense) => dense < self.data.len(),
            None => false,
        }
    }

    #[inline]
    pub(crate) fn get<C: ComponentValue>(&self, entity: Entity) -> Option<&C> {
        match self.sparse.get(entity.to_sparse_index()) {
            Some(dense) => self.data.get(*dense),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_mut<C: ComponentValue>(&mut self, entity: Entity) -> Option<&mut C> {
        match self.sparse.get(entity.to_sparse_index()) {
            Some(&dense) => self.data.get_mut(dense),
            _ => None,
        }
    }
}

pub(crate) struct TagSparseSet {
    dense: Vec<Entity>,
    sparse: Vec<usize>,
}

impl TagSparseSet {
    pub(crate) fn new() -> Self {
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

    /// Inserts the entity into the sparse set,
    /// effectively adding the tag to the entity.
    pub(crate) fn insert(&mut self, entity: Entity) {
        let sparse = entity.to_sparse_index();
        let dense = self.ensure_index(sparse);

        if dense != usize::MAX {
            debug_assert!(dense < self.dense.len(), "dense index is out of bounds");
            self.dense[dense] = entity;
        } else {
            self.sparse[sparse] = self.dense.len();
            self.dense.push(entity);
        }
    }

    /// Removes an entity from the set.
    /// Returns the value associated with the entity if it was present.
    pub(crate) fn remove<C: ComponentValue>(&mut self, entity: Entity) -> Option<Entity> {
        let sparse = entity.to_sparse_index();
        match self.sparse.get(sparse) {
            Some(&dense) if dense < self.dense.len() => {
                self.sparse[sparse] = usize::MAX;
                let entity = self.dense.swap_remove(dense);
                if dense != self.dense.len() {
                    self.sparse[self.dense[dense].to_sparse_index()] = dense;
                }
                Some(entity)
            }
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn has(&self, entity: Entity) -> bool {
        match self.sparse.get(entity.to_sparse_index()) {
            Some(&dense) => dense < self.dense.len(),
            None => false,
        }
    }
}
