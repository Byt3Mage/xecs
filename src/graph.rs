use ahash::AHashMap;

use crate::{
    component::{Signature, id::ComponentId},
    ecs::Ecs,
    storage::{
        block::Block,
        table::{Column, ColumnMap, Table, TableData},
    },
    table_index::TableId,
};

pub(crate) struct GraphNode {
    add: AHashMap<ComponentId, TableId>,
    remove: AHashMap<ComponentId, TableId>,
}

impl GraphNode {
    pub(crate) fn new() -> Self {
        Self { add: AHashMap::new(), remove: AHashMap::new() }
    }
}

fn new_table(ecs: &mut Ecs, sig: Signature) -> TableId {
    ecs.tables.add_with_id(|tid| {
        let mut col_map = ColumnMap::new();
        let mut columns = Vec::with_capacity(sig.len());

        for (i, &id) in sig.iter().enumerate() {
            let ci = &mut ecs.components[id];
            ci.insert_table(tid);
            col_map.insert(id, i);
            columns.push(Column { id, data: Block::new(ci.meta.layout, ci.meta.dtor) })
        }

        Table {
            sig,
            col_map,
            data: TableData::new(columns.into()),
            node: GraphNode::new(),
        }
    })
}

/// Traverse the table graph to find the destination table for an added component.
///
/// Returns `None` if the component is already present in the table.
pub fn find_add_table(ecs: &mut Ecs, from: TableId, with: ComponentId) -> Option<TableId> {
    let from_table = &ecs.tables[from];
    match from_table.node.add.get(&with) {
        Some(&to) => Some(to),
        None => {
            let ids = from_table.sig.try_extend(with)?;
            let to = ecs.tables.get_id(&ids).unwrap_or_else(|| new_table(ecs, ids));
            ecs.tables[from].node.add.insert(with, to); // Insert add edge From -> To
            ecs.tables[to].node.remove.insert(with, from); // Insert remove edge To -> From
            Some(to)
        }
    }
}

/// Traverse the table graph to find the destination table for a removed component.
///
/// Returns `None` if the component is not present in the table.
pub fn find_remove_table(ecs: &mut Ecs, from: TableId, without: ComponentId) -> Option<TableId> {
    let from_table = &ecs.tables[from];
    match from_table.node.remove.get(&without) {
        Some(&to) => Some(to),
        None => {
            let ids = from_table.sig.try_shrink(without)?;
            let to = ecs.tables.get_id(&ids).unwrap_or_else(|| new_table(ecs, ids));
            ecs.tables[from].node.remove.insert(without, to); // Insert remove edge From -> To
            ecs.tables[to].node.add.insert(without, from); // Insert add edge To -> From
            Some(to)
        }
    }
}
