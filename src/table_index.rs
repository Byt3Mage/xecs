use crate::graph::GraphNode;
use crate::id::map::IdMap;
use crate::storage::table::TableData;
use crate::{id::Signature, storage::table::Table};
use std::collections::hash_map::Values;
use std::{
    collections::HashMap,
    fmt::Display,
    hash::Hash,
    ops::{Index, IndexMut},
};

/// Stable, non-recycled handle into [TableIndex].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub(crate) struct TableId(u32);

impl Display for TableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TableId({})", self.0)
    }
}

impl Default for TableId {
    fn default() -> Self {
        Self::NULL
    }
}

impl TableId {
    pub(crate) const NULL: Self = Self(u32::MAX);
}

pub(crate) struct TableIndex {
    root: TableId,
    tables: Vec<Table>,
    table_ids: HashMap<Signature, TableId>,
}

impl TableIndex {
    pub(crate) fn new() -> Self {
        Self {
            root: TableId(0),
            tables: vec![Table {
                sig: Signature::from(vec![]),
                data: TableData::new(Box::new([])),
                col_map: IdMap::new(),
                graph_node: GraphNode::new(),
            }],
            table_ids: HashMap::new(),
        }
    }

    pub fn root_id(&self) -> TableId {
        self.root
    }

    pub fn root(&self) -> &Table {
        &self.tables[self.root.0 as usize]
    }

    pub fn root_mut(&mut self) -> &mut Table {
        &mut self.tables[self.root.0 as usize]
    }

    pub(crate) fn add_with_id<F>(&mut self, f: F) -> TableId
    where
        F: FnOnce(TableId) -> Table,
    {
        assert!(self.tables.len() < u32::MAX as usize);

        let id = TableId(self.tables.len() as u32);
        let table = f(id);
        self.table_ids.insert(table.sig.clone(), id);
        self.tables.push(table);
        id
    }

    #[inline]
    pub(crate) fn add(&mut self, table: Table) -> TableId {
        self.add_with_id(|_| table)
    }

    #[inline]
    pub(crate) fn get_id(&self, ids: &Signature) -> Option<TableId> {
        self.table_ids.get(ids).copied()
    }

    /// ## Panics
    /// Panics if table ids `a` and `b` are equal or if either table id is invalid.
    #[inline]
    pub(crate) fn get_2_mut(&mut self, a: TableId, b: TableId) -> [&mut Table; 2] {
        let len = self.tables.len();
        let a = a.0 as usize;
        let b = b.0 as usize;

        if a == b {
            panic!("table ids are equal (id = {a})");
        }

        if a >= len {
            panic!("table id {a} is out of bounds (len = {len})")
        }

        if b >= len {
            panic!("table id {b} is out of bounds (len = {len})")
        }

        // SAFETY: a and b are valid indices and not equal.
        let ptr = self.tables.as_mut_ptr();
        unsafe { [&mut *(ptr.add(a)), &mut *(ptr.add(b))] }
    }

    pub(crate) fn all_table_ids(&self) -> Values<'_, Signature, TableId> {
        self.table_ids.values()
    }
}

impl Index<TableId> for TableIndex {
    type Output = Table;

    #[inline(always)]
    fn index(&self, index: TableId) -> &Self::Output {
        &self.tables[index.0 as usize]
    }
}

impl IndexMut<TableId> for TableIndex {
    #[inline(always)]
    fn index_mut(&mut self, index: TableId) -> &mut Self::Output {
        &mut self.tables[index.0 as usize]
    }
}
