use crate::component::TableRecord;
use crate::table_index::TableId;
use sparse_storage::{ComponentSparseSet, TagSparseSet};
use std::collections::HashMap;

pub(crate) mod sparse_set;
pub(crate) mod sparse_storage;
pub(crate) mod table;
pub(crate) mod table_data;

mod erased_vec;

/// The type of storage used for components
#[derive(Clone, Copy, PartialEq, Hash)]
pub enum StorageType {
    /// Component data or Tag is stored in tables.
    ///
    /// # Tradeoffs
    /// - Adding or removing a table-stored component triggers an archetype move which can be expensive.
    /// - Queries with only table-stored components are fast and efficient to iterate.
    /// - Tables are the most memory-efficient storage type.
    /// - Finding a component for an entity is the slowest.
    Tables,
    /// Component data or Tag is stored in a sparse set.
    ///
    /// # Tradeoffs
    /// - Adding or removing the component is very fast.
    /// - Queries with sparse components are slower to iterate than queries with table-only components.
    /// - Sparse components waste the most memory if ids are very sparse.
    /// - Finding a component for an entity is the fastest.
    SparseSet,
    /// Component data or Tag is stored in sparse set with paged sparse arrays.
    /// (usize) represents the page size (number of entity ids per page).
    ///
    /// # Tradeoffs
    /// - Adding or removing the component on an entity is faster than tables but slower than sparse.
    ///   This is because it requires an extra lookup to find the sparse page.
    /// - Queries with paged sparse components are the slowest to iterate
    /// - Paged sparse components waste less memory than sparse components if ids are very sparse.
    /// - Finding a component for an entity is slower than sparse but faster than tables.
    PagedSparseSet(usize),
    // TODO: DenseVec (Vec<Option<C>>), entities directly index a vector,
    // wastes the most memory, slowest to iterate
    // but offers the fastest insertion and removal.
}

pub(crate) enum Storage {
    SparseTag(TagSparseSet),
    SparseData(ComponentSparseSet),
    Tables(HashMap<TableId, TableRecord>),
}
