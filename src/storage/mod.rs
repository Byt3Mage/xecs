use crate::{component::TableRecord, table_index::TableId};
use sparse_set::{SparseData, SparseTag};
use std::collections::HashMap;

pub(crate) mod column;
pub(crate) mod sparse_set;
pub(crate) mod table;
pub(crate) mod table_data;

/// The type of storage used for components
#[derive(Default, Clone, Copy, PartialEq, Hash)]
pub enum StorageType {
    /// Component data or Tag is stored in tables.
    ///
    /// # Tradeoffs
    /// - Adding or removing a table-stored component triggers an archetype move which can be expensive.
    /// - Queries with only table-stored components are fast and efficient to iterate.
    /// - Tables are the most memory-efficient storage type.
    /// - Finding a component for an entity is the slowest.
    #[default]
    Tables,
    /// Component data or Tag is stored in a sparse set.
    ///
    /// # Tradeoffs
    /// - Adding or removing the component is very fast.
    /// - Queries with sparse components are slower to iterate than queries with table-only components.
    /// - Sparse components waste the most memory if ids are very sparse.
    /// - Finding a component for an entity is the fastest.
    Sparse,
}

pub(crate) enum Storage {
    SparseTag(SparseTag),
    SparseData(SparseData),
    Tables(HashMap<TableId, TableRecord>),
}

impl Storage {
    pub(crate) fn get_type(&self) -> StorageType {
        match self {
            Storage::Tables(_) => StorageType::Tables,
            _ => StorageType::Sparse,
        }
    }
}
