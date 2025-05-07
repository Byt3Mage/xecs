use crate::{storage::table::Table, types::IdList};
use std::{
    collections::HashMap,
    fmt::Display,
    ops::{Index, IndexMut},
};

/// Stable, non-recycled handle into [TableIndex].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub(crate) struct TableId(usize);

impl Display for TableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TableId({})", self.0)
    }
}

impl TableId {
    pub(crate) const NULL: Self = Self(usize::MAX);

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
        let id = TableId(self.tables.len());
        let table = f(id);
        self.table_ids.insert(table.ids.clone(), id);
        self.tables.push(table);
        id
    }

    #[inline]
    pub(crate) fn get(&self, id: TableId) -> Option<&Table> {
        self.tables.get(id.0)
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, id: TableId) -> Option<&mut Table> {
        self.tables.get_mut(id.0)
    }

    #[inline]
    pub(crate) fn get_id(&self, ids: &IdList) -> Option<TableId> {
        self.table_ids.get(ids).copied()
    }

    #[inline]
    pub(crate) fn get_2_mut(&mut self, a: TableId, b: TableId) -> Option<(&mut Table, &mut Table)> {
        let len = self.tables.len();

        if a == b || a.0 >= len || b.0 >= len {
            None
        } else {
            let ptr = self.tables.as_mut_ptr();
            Some(unsafe { (&mut *(ptr.add(a.0)), &mut *(ptr.add(b.0))) })
        }
    }

    pub(crate) fn to_slice(&self) -> &[Table] {
        &self.tables
    }
}

impl Index<TableId> for TableIndex {
    type Output = Table;

    #[inline(always)]
    fn index(&self, index: TableId) -> &Self::Output {
        &self.tables[index.0]
    }
}

impl IndexMut<TableId> for TableIndex {
    #[inline(always)]
    fn index_mut(&mut self, index: TableId) -> &mut Self::Output {
        &mut self.tables[index.0]
    }
}
