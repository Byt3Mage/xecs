use ahash::AHashMap;

use crate::{
    component::{ComponentId, Signature},
    ecs::Ecs,
    storage::{
        blob::Blob,
        table::{Column, Table, TableData},
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

fn new_table(ecs: &mut Ecs, ids: Signature) -> TableId {
    ecs.tables.add_with_id(|table_id| {
        let mut col_map = AHashMap::new();
        let mut cols = Vec::new();

        for &id in ids.iter() {
            let ci = &mut ecs.components[id];
            col_map.insert(id, cols.len());
            ci.insert_table(table_id);
            cols.push(Column { id, data: Blob::new(ci.meta.clone()) });
        }

        Table {
            sig: ids,
            data: TableData::new(cols.into()),
            col_map,
            graph_node: GraphNode::new(),
        }
    })
}

/// Traverse the table graph to find the destination table for an added component.
///
/// Returns `None` if the component is already present in the table.
pub fn find_add_table(ecs: &mut Ecs, from: TableId, with: ComponentId) -> Option<TableId> {
    let from_table = &ecs.tables[from];

    if let Some(&to) = from_table.graph_node.add.get(&with) {
        return Some(to);
    }

    let ids = from_table.sig.try_extend(with)?;

    let to = match ecs.tables.get_id(&ids) {
        Some(id) => id,
        None => new_table(ecs, ids),
    };

    // Insert add edge From -> To
    ecs.tables[from].graph_node.add.insert(with, to);
    // Insert remove edge To -> From
    ecs.tables[to].graph_node.remove.insert(with, from);

    Some(to)
}

/// Traverse the table graph to find the destination table for a removed component.
///
/// Returns `None` if the component is not present in the table.
pub fn find_remove_table(ecs: &mut Ecs, from: TableId, without: ComponentId) -> Option<TableId> {
    let from_table = &ecs.tables[from];

    if let Some(&to) = from_table.graph_node.remove.get(&without) {
        return Some(to);
    }

    let ids = from_table.sig.try_shrink(without)?;

    let to = match ecs.tables.get_id(&ids) {
        Some(id) => id,
        None => new_table(ecs, ids),
    };

    // Insert remove edge From -> To
    ecs.tables[from].graph_node.remove.insert(without, to);
    // Insert add edge To -> From
    ecs.tables[to].graph_node.add.insert(without, from);

    Some(to)
}
