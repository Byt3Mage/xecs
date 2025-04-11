use std::{ptr, rc::Rc, usize, vec};

use crate::{entity::Entity, entity_index::EntityLocation, graph::GraphNode, id::Id, type_info::Type, world::World};
use super::{archetype_data::ArchetypeData, archetype_flags::ArchetypeFlags, archetype_index::ArchetypeId};

pub struct Archetype {
    pub id: ArchetypeId,
    pub flags: ArchetypeFlags,
    pub type_: Type, // vector of component ids.
    pub component_map: Box<[usize]>, // maps component ids to columns.
    column_map: Box<[usize]>, // maps columns to component ids.
    pub node: GraphNode,
    data: ArchetypeData,
}

/// Gets the component [Id] for the corresponding column index.
#[inline]
fn column_to_id(arch: &Archetype, column: usize) -> Id {
    arch.type_[arch.column_map[column]]
}

/// Moves entity from src archetype to dst.
/// 
/// # Safety
/// - `src_row` must be a valid row in `src`. 
/// - `asrc_id` and `adst_id` must not be the same archetype.
pub unsafe fn move_entity(world: &mut World, entity: Entity, asrc_id: ArchetypeId, src_row: usize, adst_id: ArchetypeId) {
    debug_assert!(asrc_id != adst_id, "Source and destination archetypes are the same");

    let [src, dst] = world.archetypes.get_multi_mut([asrc_id, adst_id]);
    
    debug_assert!(src_row < src.data.count(), "row out of bounds");
    
    let dst_row = unsafe { dst.data.new_row_uninit(entity) };
    let mut i_src = 0; let src_col_count = src.data.columns.len();
    let mut i_dst = 0; let dst_col_count = dst.data.columns.len();
    let mut should_drop = vec![true; src_col_count];

    // Transfer matching columns.
    while (i_src < src_col_count) && (i_dst < dst_col_count) {
        let src_id = column_to_id(&src, i_src);
        let dst_id = column_to_id(&dst, i_dst);

        if dst_id == src_id {
            let src_col = &mut src.data.columns[i_dst];
            let dst_col = &mut dst.data.columns[i_src];

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

    // Update entity location after running remove actions.
    world.entity_index.set_location(entity, EntityLocation::new(adst_id, dst_row));
}

pub(crate) fn move_entity_to_root(world: &mut World, entity: Entity) {
    debug_assert!(world.root_arch.is_null() == false, "World must initialize a root archetype");

    let EntityLocation{arch, row} = world.entity_index.get_location(entity).unwrap();

    if arch.is_null() {
        let root_arch = world.archetypes.get_mut(world.root_arch).unwrap();

        debug_assert!(root_arch.data.columns.is_empty(), "INTERNAL ERROR: root archetype should not contain columns");

        // SAFETY: root archetype should never contain columns, so only the entity array is initialized. 
        let new_row = unsafe { root_arch.data.new_row_uninit(entity) };

        world.entity_index.set_location(entity, EntityLocation::new(root_arch.id, new_row));
    }
    else if arch != world.root_arch {
        // SAFETY:
        // - row is valid in enitity index.
        // - we just checked that arch and root_arch are not the same.
        unsafe {
            move_entity(world, entity, arch, row, world.root_arch);
        }
    }
}