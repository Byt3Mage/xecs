use crate::{
    component::TableRecord,
    entity::{Entity, EntityMap},
    flags::TableFlags,
    storage::{
        Storage,
        table::Table,
        table_data::{Column, TableData},
    },
    table_index::TableId,
    types::IdList,
    world::World,
};
use std::rc::Rc;

pub(crate) struct GraphEdge {
    from: TableId,
    to: TableId,
}

pub(crate) struct GraphNode {
    add: EntityMap<GraphEdge>,
    remove: EntityMap<GraphEdge>,
}

impl GraphNode {
    pub(crate) fn new() -> Self {
        Self {
            add: EntityMap::default(),
            remove: EntityMap::default(),
        }
    }
}

fn new_table(world: &mut World, ids: IdList) -> TableId {
    world.table_index.add_with_id(|table_id| {
        let mut columns = Vec::new();
        let mut component_map = EntityMap::default();

        for (index, &id) in ids.iter().enumerate() {
            let cr = world.components.get_mut(&id).unwrap();
            let mut tr = TableRecord {
                id_index: index,
                column_index: -1,
            };

            if let Some(ti) = &cr.type_info {
                let col_idx = columns.len();
                tr.column_index = col_idx as isize;
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
            flags: TableFlags::empty(),
            ids,
            data: TableData::new(columns.into()),
            component_map,
            node: GraphNode::new(),
        }
    })
}

/// Traverse the table graph to find the destination table for a component.
///
/// Returns `None` if the component is already present.
pub fn table_traverse_add(world: &mut World, from_id: TableId, with: Entity) -> Option<TableId> {
    let from = &world.table_index[from_id];

    if let Some(edge) = from.node.add.get(&with) {
        debug_assert_eq!(edge.from, from.id);
        debug_assert_ne!(edge.to, from.id);
        return Some(edge.to);
    }

    // Edge doesn't exist, find new table and create one.
    if let Some(ty) = from.ids.try_extend(with) {
        let to_id = match world.table_index.get_id(&ty) {
            Some(id) => id,
            None => new_table(world, ty),
        };

        //TODO: consider using pointers to avoid double indexing.
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

    return None;
}
