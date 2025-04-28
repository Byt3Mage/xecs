use std::{collections::HashMap, ptr::NonNull};

use crate::{
    entity::Entity,
    flags::TableFlags,
    storage::{
        table::Table,
        table_index::{TableBuilder, TableId},
    },
    type_info::Type,
    world::World,
};

pub struct TableDiff {
    added: Type,
    removed: Type,
    added_flags: TableFlags,
    removed_flags: TableFlags,
}

pub struct GraphEdge {
    pub from: NonNull<Table>,
    pub to: NonNull<Table>,
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
}

fn table_ensure_edge<'a>(
    world: &mut World,
    edges: &'a mut GraphEdges,
    id: Entity,
) -> &'a mut GraphEdge {
    todo!()
}

fn new_table(world: &mut World, ty: Type) -> TableId {
    TableBuilder::new(ty).build(world)
}

fn ensure_table(world: &mut World, ty: Type) -> TableId {
    if ty.id_count() == 0 {
        world.root_table
    } else {
        world
            .table_index
            .get_id(&ty)
            .unwrap_or_else(|| new_table(world, ty))
    }
}

/// Traverse the table graph to find the destination table for a component.
///
/// Returns the source table if the component is already present.
///
/// TODO: use table graph/diff to find the destination table.
pub fn table_traverse_add(world: &mut World, from: NonNull<Table>, with: Entity) -> NonNull<Table> {
    todo!()
}

fn find_table_with_id(world: &mut World, node: TableId, with: Entity) -> TableId {
    todo!()
}
