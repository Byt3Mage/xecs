use crate::table_index::TableId;
use ahash::AHashSet;
use sparse::SparseSet;

pub(crate) mod column;
pub(crate) mod resource;
pub(crate) mod sparse;
pub(crate) mod table;

mod erased_vec;

/// The type of storage used for components
#[derive(Debug, Default, Clone, Copy, PartialEq, Hash)]
pub enum StorageType {
    /// Component data or Tag is stored in tables.
    ///
    /// # Tradeoffs
    /// - Adding or removing a component triggers an expensive archetype move
    /// - Tables are the most memory-efficient storage type
    /// - Finding a component for an entity is slower than sparse
    /// - Queries with only table-stored components are very fast to iterate
    #[default]
    Tables,
    /// Component data or Tag is stored in a sparse set.
    ///
    /// # Tradeoffs
    /// - Adding or removing the component is very fast
    /// - Sparse components waste memory if ids are very sparse
    /// - Finding a component for an entity is the fastest
    /// - Queries with sparse components are slower to iterate than queries with table-only components
    Sparse,
}

#[derive(Debug)]
pub(crate) enum Storage {
    Sparse(SparseSet),
    Tables(AHashSet<TableId>),
}
