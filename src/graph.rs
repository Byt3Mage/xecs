use crate::{
    component::ComponentLocation,
    flags::TableFlags,
    id::{Entity, Id, IdMap, Signature},
    storage::{
        Storage,
        table::{Column, Table, TableData},
    },
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
            add: IdMap::new(256),
            remove: IdMap::new(256),
        }
    }
}

fn new_table(world: &mut World, ids: Signature) -> TableId {
    world.tables.add_with_id(|table_id| {
        let mut columns = Vec::new();
        let mut component_map = IdMap::new(256);

        for (index, &id) in ids.iter().enumerate() {
            let cr = id.map_get_mut(&mut world.components).unwrap();
            let mut cl = ComponentLocation {
                id_idx: index,
                col_idx: None,
            };

            if let Some(ti) = &cr.type_info {
                let col_idx = columns.len();
                cl.col_idx = Some(col_idx);
                id.map_insert(&mut component_map, col_idx);
                columns.push(Column::new(id, Rc::clone(ti)));
            }

            match &mut cr.storage {
                Storage::Tables(tables) => tables.insert(table_id, cl),
                _ => panic!("INTERNAL ERROR: unexpected storage type"),
            };
        }

        Table {
            id: table_id,
            _flags: TableFlags::empty(),
            signature: ids,
            data: TableData::new(columns.into()),
            column_map: component_map,
            node: GraphNode::new(),
        }
    })
}

/// Traverse the table graph to find the destination table for an added component.
///
/// Returns `None` if the component is already present.
pub fn table_traverse_add(world: &mut World, from_id: TableId, with: Entity) -> Option<TableId> {
    let from = &world.tables[from_id];

    if let Some(edge) = with.map_get(&from.node.add) {
        return Some(edge.to);
    }

    let ids = from.signature.try_extend(with)?;
    let to_id = match world.tables.get_id(&ids) {
        Some(id) => id,
        None => new_table(world, ids),
    };

    let from = &mut world.tables[from_id];

    with.map_insert(
        &mut from.node.add,
        GraphEdge {
            from: from_id,
            to: to_id,
        },
    );

    Some(to_id)
}
