use crate::{
    flags::TableFlags, graph::GraphNode, storage::table::Table, type_info::Type, world::World,
};
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
    table_ids: HashMap<Type, TableId>,
}

impl TableIndex {
    pub(crate) fn new() -> Self {
        Self {
            tables: Vec::new(),
            table_ids: HashMap::new(),
        }
    }

    fn add_with_id<F>(&mut self, f: F) -> TableId
    where
        F: FnOnce(TableId) -> Table,
    {
        let id = TableId(self.tables.len());
        let table = f(id);
        self.table_ids.insert(table.type_.clone(), id);
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
    pub(crate) fn get_id(&self, type_: &Type) -> Option<TableId> {
        self.table_ids.get(type_).copied()
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

pub(crate) struct TableBuilder {
    flags: TableFlags,
    type_: Type,
    node: GraphNode,
}

impl TableBuilder {
    pub(crate) fn new(type_ids: Type) -> Self {
        Self {
            flags: TableFlags::default(),
            type_: type_ids,
            node: GraphNode::new(),
        }
    }

    pub(crate) fn with_flags(mut self, flags: TableFlags) -> Self {
        self.flags |= flags;
        self
    }

    pub(crate) fn build(self, world: &mut World) -> TableId {
        /*world.table_index.add_with_id(|table_id| {
            let count = self.type_.id_count();
            let mut columns = Vec::with_capacity(count);
            let mut component_map_lo = [-1; HI_COMPONENT_ID as usize];
            let mut component_map_hi = HashMap::new();

            for (idx, &id) in self.type_.iter().enumerate() {
                let cr = world
                    .get_component(id)
                    .expect("Component record not found.");
                let mut cl = ComponentLocation {
                    id_index: idx,
                    id_count: 1,
                    column_index: -1,
                };

                // Component contains type_info, initialize a column for it.
                if let Some(ti) = &cr.type_info {
                    let col_idx = columns.len();

                    cl.column_index = col_idx as isize;

                    if id < HI_COMPONENT_ID {
                        component_map_lo[id as usize] = col_idx as isize;
                    } else {
                        component_map_hi.insert(id, col_idx);
                    }

                    columns.push(Column::new(id, Rc::clone(ti)));
                }
            }

            Table {
                id: table_id,
                flags: self.flags,
                type_: self.type_,
                component_map_lo,
                component_map_hi,
                node: self.node,
                data: TableData::new(columns.into()),
            }
        })*/

        todo!()
    }
}
