use std::{collections::HashMap, ptr::NonNull};

use crate::{
    entity::{ECS_CHILD_OF, ECS_DISABLED, ECS_IS_A, ECS_MODULE, ECS_NOT_QUERYABLE, ECS_PREFAB},
    flags::{ComponentFlags, TableFlags},
    id::{ECS_AUTO_OVERRIDE, ECS_TOGGLE, Id, has_id_flag, is_pair, pair_first, pair_second},
    storage::{table::Table, table_index::TableBuilder},
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
    pub id: Id,
    /// Added/Removed components between tables
    pub diff: Option<TableDiff>,
}

pub struct GraphEdges {
    lo: Vec<GraphEdge>,
    hi: HashMap<Id, GraphEdge>,
}

pub struct GraphNode {
    pub add: GraphEdges,
    pub remove: GraphEdges,
}

fn table_ensure_edge<'a>(
    world: &mut World,
    edges: &'a mut GraphEdges,
    id: Id,
) -> &'a mut GraphEdge {
    todo!()
}

fn init_table_flags(world: &World, ty: &Type) -> TableFlags {
    let mut flags = TableFlags::empty();

    for &id in ty.ids().iter() {
        if id == ECS_MODULE {
            flags |= TableFlags::HAS_BUILTINS;
            flags |= TableFlags::HAS_MODULE;
        } else if id == ECS_PREFAB {
            flags |= TableFlags::IS_PREFAB;
        } else if id == ECS_DISABLED {
            flags |= TableFlags::IS_DISABLED;
        } else if id == ECS_NOT_QUERYABLE {
            flags |= TableFlags::NOT_QUERYABLE;
        } else {
            if is_pair(id) {
                let r = pair_first(id) as u64;
                flags |= TableFlags::HAS_PAIRS;

                if r == ECS_IS_A {
                    flags |= TableFlags::HAS_IS_A;
                } else if r == ECS_CHILD_OF {
                    flags |= TableFlags::HAS_CHILD_OF;

                    let tgt = world.entity_index.get_current(pair_second(id) as u64);
                    assert!(tgt != 0);

                    if world.has(tgt, ECS_MODULE) {
                        /* If table contains entities that are inside one of the
                         * builtin modules, it contains builtin entities */
                        flags |= TableFlags::HAS_BUILTINS;
                        flags |= TableFlags::HAS_MODULE;
                    }
                }
            } else {
                if has_id_flag(id, ECS_TOGGLE) {
                    flags |= TableFlags::HAS_TOGGLE;
                }

                if has_id_flag(id, ECS_AUTO_OVERRIDE) {
                    flags |= TableFlags::HAS_OVERRIDES;
                }
            }
        }
    }

    flags
}

fn new_table(world: &mut World, ty: Type) -> NonNull<Table> {
    let flags = init_table_flags(world, &ty);
    let table = TableBuilder::new(ty.clone()).with_flags(flags).build(world);
    world.table_map.insert(ty, table);
    table
}

fn ensure_table(world: &mut World, ty: Type) -> NonNull<Table> {
    if ty.id_count() == 0 {
        world.root_table
    } else {
        world
            .table_map
            .get(&ty)
            .copied()
            .unwrap_or_else(|| new_table(world, ty))
    }
}

/// Traverse the table graph to find the destination table for a component.
///
/// Returns the source table if the component is already present.
///
/// TODO: use table graph/diff to find the destination table.
pub fn table_traverse_add(world: &mut World, from: NonNull<Table>, with: Id) -> NonNull<Table> {
    todo!()
}

fn find_table_with_id(world: &mut World, mut node: NonNull<Table>, with: Id) -> NonNull<Table> {
    let cr = world.components.get(with).unwrap();
    let node_ref = unsafe { node.as_mut() };

    if cr.flags.contains(ComponentFlags::IS_SPARSE) {
        node_ref.flags.insert(TableFlags::HAS_SPARSE);
        return node;
    }

    node_ref
        .type_
        .extend_with(with)
        .map_or(node, |ty| ensure_table(world, ty))
}
