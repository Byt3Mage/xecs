use std::{collections::HashMap, rc::Rc};

use crate::{entity::Entity, flags::ArchetypeFlags, graph::GraphNode, id::Id, type_info::Type, world::World};
use super::{ArchetypeData, archetype_index::ArchetypeId};

pub struct Archetype {
    /// Handle to self in [ArchetypeIndex](super::archetype_index::ArchetypeIndex).
    pub(crate) id: ArchetypeId,
    /// Flags describing capabilites of this archetype
    pub(crate) flags: ArchetypeFlags,
    /// Vector of component [Id]s
    pub(crate) type_: Type,
    /// Maps component ids to columns.
    pub(crate) component_map: HashMap<Id, usize>,
    pub(crate) node: GraphNode,
    /// Storage for entities and components.
    pub(crate) data: ArchetypeData,
    /// Number of traversable entities in this archetype.
    pub(crate) traversable_count: usize,
}

/// Moves entity from src archetype to dst.
/// 
/// # Safety
/// - `src_row` must be a valid row in `src`. 
/// - `asrc_id` and `adst_id` must not be the same archetype.
pub(crate) unsafe fn move_entity(world: &mut World, entity: Entity, src: ArchetypeId, src_row: usize, dst: ArchetypeId) -> usize {
    let (src, dst) = world.archetypes.get_two_mut(src, dst);

    debug_assert!(src_row < src.data.count(), "row out of bounds");
    
    let dst_row = unsafe { dst.data.new_row_uninit(entity) };

    let mut i_src = 0; let src_col_count = src.data.columns.len();
    let mut i_dst = 0; let dst_col_count = dst.data.columns.len();
    let mut should_drop = vec![true; src_col_count];

    // Transfer matching columns.
    while (i_src < src_col_count) && (i_dst < dst_col_count) {
        let src_col = &mut src.data.columns[i_src];
        let dst_col = &mut dst.data.columns[i_dst];

        let src_id = src_col.id();
        let dst_id = dst_col.id();    

        if dst_id == src_id {
            debug_assert!(Rc::ptr_eq(&dst_col.type_info, &src_col.type_info), "INTERNAL ERROR: Type mismatch");

            let ti = &dst_col.type_info;
            let size = ti.size();
            let move_fn = ti.hooks.move_fn;

            // SAFETY:
            // - caller guarantees that src_row and dst_row are valid indices.
            // - caller ensures that move_fn implementation properly follows move semantics.
            // - src_elem and dst_elem are valid pointers to the same type.
            unsafe {
                let src_elem = src_col.data.add(src_row * size);
                let dst_elem = dst_col.data.add(dst_row * size);
                move_fn(src_elem, dst_elem);
            }

            // Don't call drop on this column since we have moved the value.
            should_drop[i_src] = false;
        }
        else if dst_id < src_id {
            //invoke_add_hooks(world, dst, dst_col, &dst_entity, dst_row);
        }
        
        i_dst += (dst_id <= src_id) as usize;
        i_src += (dst_id >= src_id) as usize;
    }

    while i_dst < dst_col_count {
        // invoke_add_hooks
        i_dst += 1;
    }

    while i_src < src_col_count {
        // invoke_remove_hook
        i_src += 1;
    }

    src.data.delete_row(&mut world.entity_index, src_row, should_drop);
    world.entity_index.set_location(entity, dst.id, dst_row);

    dst_row
}

pub(crate) fn move_entity_to_root(world: &mut World, entity: Entity) {
    let (arch, row) = world.entity_index.get_location(entity).unwrap();

    if arch.is_null() {
        let root_arch = world.archetypes.get_mut(world.root_arch).expect("INTERNAL ERROR: world must initialize root archetype");
        
        // SAFETY: we guarantee root archetype should never contain columns, 
        // so only the entities array is initialized. 
        let new_row = unsafe { root_arch.data.new_row_uninit(entity) };

        world.entity_index.set_location(entity, root_arch.id, new_row);
    }
    else if arch != world.root_arch {
        // SAFETY:
        // - row is valid in enitity index.
        // - we just checked that arch and root_arch are not the same.
        unsafe { move_entity(world, entity, arch, row, world.root_arch); }
    }
}