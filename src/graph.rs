use crate::{
    component::TableRecord,
    flags::TableFlags,
    id::{Id, IdList, IdMap},
    storage::{Storage, column::Column, table::Table, table_data::TableData},
    table_index::TableId,
    world::World,
};
use std::rc::Rc;

#[derive(Default)]
pub(crate) struct GraphEdge {
    from: TableId,
    to: TableId,
}

pub(crate) struct GraphNode {
    add: IdMap<GraphEdge>,
    remove: IdMap<GraphEdge>,
}

impl GraphNode {
    pub(crate) fn new() -> Self {
        Self {
            add: IdMap::new(),
            remove: IdMap::new(),
        }
    }
}

fn new_table(world: &mut World, ids: IdList) -> TableId {
    world.table_index.add_with_id(|table_id| {
        let mut columns = Vec::new();
        let mut component_map = IdMap::new();

        for (index, &id) in ids.iter().enumerate() {
            let cr = world.components.get_mut(id).unwrap();
            let mut tr = TableRecord {
                id_idx: index,
                col_idx: None,
            };

            if let Some(ti) = &cr.type_info {
                let col_idx = columns.len();
                tr.col_idx = Some(col_idx);
                component_map.insert(id, col_idx);
                columns.push(Column::new(id, Rc::clone(ti)));
            }

            match &mut cr.storage {
                Storage::Tables(tables) => tables.insert(table_id, tr),
                _ => panic!("INTERNAL ERROR: unexpected storage type"),
            };
        }

        Table {
            id: table_id,
            _flags: TableFlags::empty(),
            ids,
            data: TableData::new(columns.into()),
            component_map,
            node: GraphNode::new(),
        }
    })
}

/// Traverse the table graph to find the destination table for an added component.
///
/// Returns `None` if the component is already present.
pub fn table_traverse_add(world: &mut World, from_id: TableId, with: Id) -> Option<TableId> {
    let from = &world.table_index[from_id];

    if let Some(edge) = from.node.add.get(with) {
        return Some(edge.to);
    }

    let ids = from.ids.try_extend(with)?;
    let to_id = match world.table_index.get_id(&ids) {
        Some(id) => id,
        None => new_table(world, ids),
    };

    let from = &mut world.table_index[from_id];

    from.node.add.insert(
        with,
        GraphEdge {
            from: from_id,
            to: to_id,
        },
    );

    return Some(to_id);
}
