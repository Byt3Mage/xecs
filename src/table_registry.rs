use std::{
    collections::hash_map::Entry,
    hash::Hash,
    ops::{Index, IndexMut},
};

use ahash::AHashMap;

use crate::{
    component::{registry::ComponentRegistry, signature::Signature},
    macros::{declare_id, map},
    table::{self, ColumnMap, Table, TableData},
};

declare_id!(TableId, "T#");

pub(crate) struct TableRegistry {
    tables: Vec<Table>,
    table_ids: AHashMap<Signature, TableId>,
}

impl TableRegistry {
    pub const ROOT: TableId = unsafe { TableId::new_unchecked(0) };

    pub(crate) fn new() -> Self {
        Self {
            tables: vec![Table {
                sig: Signature::from([]),
                data: TableData::new(Box::new([])),
                col_map: ColumnMap::new(),
            }],
            table_ids: map![],
        }
    }

    pub(crate) fn get_or_create(&mut self, components: &mut ComponentRegistry, ids: Signature) -> TableId {
        match self.table_ids.entry(ids) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                let id = TableId::new(self.tables.len() as u32).expect("too many tables");
                self.tables.push(table::new_table(id, components, e.key().clone()));
                *e.insert(id)
            }
        }
    }

    /// ## Panics
    /// Panics if table ids `a` and `b` are equal or if either id is invalid.
    #[inline]
    pub(crate) fn get_2(&self, a: TableId, b: TableId) -> (&Table, &Table) {
        let len = self.tables.len();
        let a = a.index();
        let b = b.index();

        #[cold]
        fn validate(a: usize, b: usize, len: usize) {
            if a == b {
                panic!("table ids are equal (id = {a})");
            }

            if a >= len {
                panic!("table id {a} is out of bounds (len = {len})")
            }

            if b >= len {
                panic!("table id {b} is out of bounds (len = {len})")
            }
        }

        validate(a, b, len);

        // SAFETY: a and b are valid indices and not equal.
        let ptr = self.tables.as_ptr();
        unsafe { (&*(ptr.add(a)), &*(ptr.add(b))) }
    }

    /// ## Panics
    /// Panics if table ids `a` and `b` are equal or if either id is invalid.
    #[inline]
    pub(crate) fn get_2_mut(&mut self, a: TableId, b: TableId) -> (&mut Table, &mut Table) {
        let len = self.tables.len();
        let a = a.index();
        let b = b.index();

        #[cold]
        fn validate(a: usize, b: usize, len: usize) {
            if a == b {
                panic!("table ids are equal (id = {a})");
            }

            if a >= len {
                panic!("table id {a} is out of bounds (len = {len})")
            }

            if b >= len {
                panic!("table id {b} is out of bounds (len = {len})")
            }
        }

        validate(a, b, len);

        // SAFETY: a and b are valid indices and not equal.
        let ptr = self.tables.as_mut_ptr();
        unsafe { (&mut *(ptr.add(a)), &mut *(ptr.add(b))) }
    }

    pub(crate) fn ids(&self) -> impl Iterator<Item = TableId> {
        // SAFETY: All ids up to table.len() are non-max.
        (0..self.tables.len() as u32).map(|id| unsafe { TableId::new_unchecked(id) })
    }
}

impl Index<TableId> for TableRegistry {
    type Output = Table;

    #[inline(always)]
    fn index(&self, index: TableId) -> &Self::Output {
        &self.tables[index.index()]
    }
}

impl IndexMut<TableId> for TableRegistry {
    #[inline(always)]
    fn index_mut(&mut self, index: TableId) -> &mut Self::Output {
        &mut self.tables[index.index()]
    }
}
