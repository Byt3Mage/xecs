use crate::{storage::table::Table, types::IdList};
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

impl TableId {
    pub(crate) const NULL: Self = Self(u32::MAX);

    pub(crate) const fn is_null(&self) -> bool {
        self.0 == Self::NULL.0
    }
}

pub(crate) struct TableIndex {
    tables: Vec<Table>,
    table_ids: HashMap<IdList, TableId>,
}

impl TableIndex {
    pub(crate) fn new() -> Self {
        Self {
            tables: Vec::new(),
            table_ids: HashMap::new(),
        }
    }

    pub(crate) fn add_with_id<F>(&mut self, f: F) -> TableId
    where
        F: FnOnce(TableId) -> Table,
    {
        assert!(self.tables.len() < u32::MAX as usize);

        let id = TableId(self.tables.len() as u32);
        let table = f(id);
        self.table_ids.insert(table.ids.clone(), id);
        self.tables.push(table);
        id
    }

    #[inline]
    pub(crate) fn get_id(&self, ids: &IdList) -> Option<TableId> {
        self.table_ids.get(ids).copied()
    }

    #[inline]
    pub(crate) fn get_2_mut(&mut self, a: TableId, b: TableId) -> Option<(&mut Table, &mut Table)> {
        let len = self.tables.len();
        let a = a.0 as usize;
        let b = b.0 as usize;

        if a == b || a >= len || b >= len {
            None
        } else {
            let ptr = self.tables.as_mut_ptr();
            Some(unsafe { (&mut *(ptr.add(a)), &mut *(ptr.add(b))) })
        }
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
