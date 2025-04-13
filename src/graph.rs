use std::{collections::HashMap, rc::Rc};

use crate::{component::ComponentLocation, entity::{Entity, ECS_CHILD_OF, ECS_DISABLED, ECS_IS_A, ECS_MODULE, ECS_NOT_QUERYABLE, ECS_PREFAB}, entity_index::EntityLocation, id::{self, has_id_flag, has_relation, is_pair, is_wildcard, pair_first, pair_second, Id, ECS_AUTO_OVERRIDE, ECS_TOGGLE}, storage::{archetype::Archetype, archetype_data::Column, archetype_flags::ArchetypeFlags, archetype_index::{ArchetypeBuilder, ArchetypeId}}, type_info::{self, Type}, world::World};

pub struct ArchetypeDiff {
    added: Type,
    removed: Type,
    added_flags: ArchetypeFlags,
    removed_flags: ArchetypeFlags,
}

impl ArchetypeDiff {
    fn compute(world: &World, node: &Archetype, next: &Archetype, mut id: Id) -> Option<Self> {
        let node_type = node.type_.ids();
        let next_type = next.type_.ids();
        
        let mut i_node = 0; let node_count = node_type.len();
        let mut i_next = 0; let next_count = next_type.len();

        let mut added_count = 0;
        let mut removed_count = 0;

        let mut added_flags = ArchetypeFlags::empty();
        let mut removed_flags = ArchetypeFlags::empty();
        
        let mut trivial_edge = !has_relation(id, ECS_IS_A);
        
        /* First do a scan to see how big the diff is, so we don't have to realloc
        * or alloc more memory than required. */

        while i_node < next_count && i_next < next_count {
            let id_node = node_type[i_node];
            let id_next = next_type[i_next];

            let added = id_next < id_node;
            let removed = id_node < id_next;

            trivial_edge &= !added || id_next == id;
            trivial_edge &= !removed || id_node == id;

            if added {
                //TODO: added_flags |= flecs_id_flags_get(world, id_next) & EcsTableAddEdgeFlags;
                added_count += 1;
            }

            if removed {
                //TODO: removed_flags |= flecs_id_flags_get(world, id_next) & EcsTableAddEdgeFlags;
                removed_count += 1;
            }

            if id_node <=id_next {
                i_node += 1;
            }

            if id_next <= id_node {
                i_next += 1;
            }
        }

        while i_next < next_count {
            // TODO: added_flags |= flecs_id_flags_get(world, ids_next[i_next]) & EcsTableAddEdgeFlags;
            i_next += 1;
        }

        while i_node < node_count {
            // TODO: removed_flags |= flecs_id_flags_get(world, ids_node[i_node]) & EcsTableRemoveEdgeFlags;
            i_node += 1;
        }
        
        trivial_edge &= 
        (added_count + removed_count) <= 1 && 
        !is_wildcard(id) && 
        ((added_flags|removed_flags) == ArchetypeFlags::empty());

        if trivial_edge {
            /* If edge is trivial there's no need to create a diff element for it */
            return None;
        }

        //ecs_table_diff_builder_t *builder = &world->allocators.diff_builder;
        let added_offset = 0;//builder->added.count;
        let removed_offset = 0; //builder->removed.count;

        i_node = 0; i_next = 0;

        while i_node < node_count && i_next < next_count {
            let id_node = node_type[i_node];
            let id_next = next_type[i_next];

            if id_next < id_node {
                //flecs_diff_insert_added(world, builder, id_next);
            } 
            else if id_node < id_next {
                //flecs_diff_insert_removed(world, builder, id_node);
            }

            if id_node <= id_next {
                i_node += 1;
            }

            if id_next <=id_node {
                i_next += 1;
            }
        }

        while i_next < next_count {
            //flecs_diff_insert_added(world, builder, ids_next[i_next]);
            i_next += 1;
        }

        while i_node < node_count {
            //flecs_diff_insert_removed(world, builder, ids_node[i_node]);
            i_node +=1;
        }
        /*
        ecs_table_diff_t *diff = flecs_bcalloc(&world->allocators.table_diff);
        edge->diff = diff;
        flecs_table_diff_build(world, builder, diff, added_offset, removed_offset);
        diff->added_flags = added_flags;
        diff->removed_flags = removed_flags;

        let diff = Self {
            added_flags,
            removed_flags,
        };

        assert!(diff.added.len() == added_count);
        assert!(diff.removed.len() == removed_count);

        diff
        */
        todo!()
    }
}
pub struct GraphEdge {
    pub from: ArchetypeId,
    pub to: ArchetypeId,
    /// Component/Tag/Pair id associated with edge
    pub id: Id,
    /// Added/Removed components between archetypes
    pub diff: Option<ArchetypeDiff>
}

pub struct GraphEdges {
    edges: HashMap<Id, GraphEdge>
}

pub struct GraphNode {
    pub add: GraphEdges,
    pub remove: GraphEdges,
}

impl GraphNode {
    fn create_add_edge(&mut self, id: Id, from: ArchetypeId, to: ArchetypeId) {
        self.add.edges.entry(id).or_insert_with_key(|&id| {
            GraphEdge {
                from,
                to,
                id,
                diff: None
            }
        });
    }

    fn create_remove_edge(&mut self, id: Id, from: ArchetypeId, to: ArchetypeId) {
        self.remove.edges.entry(id).or_insert_with_key(|&id| {
            GraphEdge {
                from,
                to,
                id,
                diff: None
            }
        });
    }
}

impl GraphNode {
    pub fn new() -> Self {
        Self {
            add: GraphEdges { edges: HashMap::new() },
            remove: GraphEdges { edges: HashMap::new() }
        }
    }
}

fn init_archetype_flags(world: &World, ty: &Type) -> ArchetypeFlags {
    let mut flags = ArchetypeFlags::empty();

    for &id in ty.ids().iter() {
        if id == ECS_MODULE { 
            flags |= ArchetypeFlags::HAS_BUILTINS; 
            flags |= ArchetypeFlags::HAS_MODULE;
        }
        else if id == ECS_PREFAB {
            flags |= ArchetypeFlags::IS_PREFAB;
        }
        else if id == ECS_DISABLED {
            flags |= ArchetypeFlags::IS_DISABLED;
        }
        else if id == ECS_NOT_QUERYABLE {
            flags |= ArchetypeFlags::NOT_QUERYABLE;
        }
        else {
            if is_pair(id) {
                let r = pair_first(id);
                flags |= ArchetypeFlags::HAS_PAIRS;
                
                if r == ECS_IS_A {
                    flags |= ArchetypeFlags::HAS_IS_A;
                }
                else if r == ECS_CHILD_OF {
                    flags |= ArchetypeFlags::HAS_CHILD_OF;
                    
                    let tgt = world.entity_index.get_alive(pair_second(id));
                    assert!(tgt != 0);
                    
                    if world.has(tgt, ECS_MODULE)
                    {
                        /* If table contains entities that are inside one of the 
                         * builtin modules, it contains builtin entities */
                        flags |= ArchetypeFlags::HAS_BUILTINS;
                        flags |= ArchetypeFlags::HAS_MODULE;
                    }
                }
            }
            else {
                if has_id_flag(id, ECS_TOGGLE) {
                    flags |= ArchetypeFlags::HAS_TOGGLE;
                }

                if has_id_flag(id, ECS_AUTO_OVERRIDE) {
                    flags |= ArchetypeFlags::HAS_OVERRIDES;
                }
            }
        }
    }

    flags
}

fn new_archetype(world: &mut World, ty: Type) -> ArchetypeId {
    let flags = init_archetype_flags(world, &ty);
    ArchetypeBuilder::new(world, ty.clone()).with_flags(flags).build()
}

pub fn ensure_archetype(world: &mut World, ty: Type) -> ArchetypeId {
    if ty.id_count() == 0 {
        world.root_arch
    }
    else {
        world.archetype_map.get(&ty).copied().unwrap_or_else(||new_archetype(world, ty))
    }
}

/// Traverse the archetype graph to find the destination archetype for a component.
/// 
/// Returns the source archetype if the component is already present.
/// 
/// TODO: use archetype graph/diff to find the destination archetype.
pub fn archetype_traverse_add(world: &mut World, from_id: ArchetypeId, with: Id) -> ArchetypeId {
    let from_arch = world.archetypes.get(from_id).expect("INTERNAL ERROR: archetype not found");
    from_arch.type_.extend_with(with).map_or(from_arch.id, |ty| ensure_archetype(world, ty))
}