use crate::{
    component::TableRecord,
    entity::Entity,
    flags::TableFlags,
    storage::{
        table::Table,
        table_data::{Column, TableData},
        table_index::TableId,
    },
    type_info::Type,
    world::World,
};
use std::{collections::HashMap, rc::Rc};

pub struct TableDiff {
    added: Type,
    removed: Type,
    added_flags: TableFlags,
    removed_flags: TableFlags,
}

pub struct GraphEdge {
    pub from: TableId,
    pub to: TableId,
    /// Component/Tag/Pair id associated with edge
    pub id: Entity,
    /// Added/Removed components between tables
    pub diff: Option<TableDiff>,
}

pub struct GraphEdges {
    lo: Vec<GraphEdge>,
    hi: HashMap<Entity, GraphEdge>,
}

impl GraphEdges {
    pub fn new() -> Self {
        Self {
            lo: Vec::new(),
            hi: HashMap::new(),
        }
    }
}

pub struct GraphNode {
    pub add: GraphEdges,
    pub remove: GraphEdges,
}

impl GraphNode {
    pub fn new() -> Self {
        Self {
            add: GraphEdges::new(),
            remove: GraphEdges::new(),
        }
    }

    fn find_add_edge(&self, id: Entity) -> Option<&GraphEdge> {
        self.add.hi.get(&id)
    }

    fn find_remove_edge(&self, id: Entity) -> Option<&GraphEdge> {
        self.remove.hi.get(&id)
    }
}

fn table_ensure_edge<'a>(
    world: &mut World,
    edges: &'a mut GraphEdges,
    id: Entity,
) -> &'a mut GraphEdge {
    todo!()
}

fn new_table(world: &mut World, type_: Type) -> TableId {
    world.table_index.add_with_id(|table_id| {
        let mut columns = Vec::new();
        let mut component_map_lo = [-1; Entity::HI_COMPONENT_ID.as_usize()];
        let mut component_map_hi = HashMap::new();

        for (index, &id) in type_.iter().enumerate() {
            let cr = world.components.get_mut(&id).unwrap();
            let mut tr = TableRecord {
                id_index: index,
                column_index: -1,
            };

            if let Some(ti) = cr.type_info {
                let col_idx = columns.len();
                columns.push(Column::new(id, ti));

                if id < Entity::HI_COMPONENT_ID {
                    component_map_lo[id.as_usize()] = col_idx as isize;
                } else {
                    component_map_hi.insert(id, col_idx);
                }

                tr.column_index = col_idx as isize;
            }

            match &mut cr.storage {
                crate::storage::Storage::Tables(tables) => tables.insert(table_id, tr),
                _ => panic!("INTERNAL ERROR: Unexpected storage type"),
            };
        }

        Table {
            id: table_id,
            flags: TableFlags::empty(),
            type_,
            data: TableData::new(columns.into()),
            component_map_lo,
            component_map_hi,
            node: GraphNode::new(),
        }
    })
}

fn ensure_table(world: &mut World, ty: Type) -> TableId {
    if ty.id_count() == 0 {
        world.root_table
    } else {
        match world.table_index.get_id(&ty) {
            Some(id) => id,
            None => new_table(world, ty),
        }
    }
}

/// Traverse the table graph to find the destination table for a component.
///
/// Returns the source table if the component is already present.
///
/// TODO: use table graph/diff to find the destination table.
pub fn table_traverse_add(world: &mut World, from_id: TableId, with: Entity) -> TableId {
    let from = &world.table_index[from_id];
    match from.type_.extend_with(with) {
        Some(new_type) => ensure_table(world, new_type),
        None => from_id,
    }
}

fn find_table_with_id(world: &mut World, node: TableId, with: Entity) -> TableId {
    todo!()
}
