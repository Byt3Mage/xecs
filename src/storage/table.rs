use std::{collections::HashMap, ptr::NonNull, rc::Rc};

use super::{TableData, table_index::TableId};
use crate::{
    entity::Entity,
    flags::TableFlags,
    graph::GraphNode,
    id::{HI_COMPONENT_ID, Id},
    type_info::Type,
    world::World,
};

pub(crate) type TablePtr = Option<NonNull<Table>>;

pub(crate) struct Table {
    /// Handle to self in [tableIndex](super::table_index::tableIndex).
    pub(crate) id: TableId,
    /// Flags describing capabilites of this table
    pub(crate) flags: TableFlags,
    /// Vector of component [Id]s
    pub(crate) type_: Type,
    /// Maps component ids to columns (fast path).
    pub(crate) component_map_lo: [isize; HI_COMPONENT_ID as usize],
    /// Maps component ids to columns (slow path).
    pub(crate) component_map_hi: HashMap<Id, usize>,
    /// Node representation for traversals.
    pub(crate) node: GraphNode,
    /// Storage for entities and components.
    pub(crate) data: TableData,
    /// Number of traversable entities in this table.
    pub(crate) traversable_count: usize,
}

/// Moves entity from src table to dst.
///
/// # Safety
/// - `src_row` must be a valid row in `src`.
/// - `src` and `dst` must not be the same table.
pub(crate) unsafe fn move_entity(
    world: &mut World,
    entity: Entity,
    src: &mut Table,
    src_row: usize,
    dst: &mut Table,
) -> usize {
    debug_assert!(src_row < src.data.count(), "row out of bounds");

    let dst_row = unsafe { dst.data.new_row_uninit(entity) };

    let mut i_src = 0;
    let mut i_dst = 0;
    let src_col_count = src.data.columns.len();
    let dst_col_count = dst.data.columns.len();
    let mut should_drop = vec![true; src_col_count];

    // Transfer matching columns.
    while (i_src < src_col_count) && (i_dst < dst_col_count) {
        let src_col = &mut src.data.columns[i_src];
        let dst_col = &mut dst.data.columns[i_dst];

        let src_id = src_col.id();
        let dst_id = dst_col.id();

        if dst_id == src_id {
            debug_assert!(
                Rc::ptr_eq(&dst_col.type_info, &src_col.type_info),
                "INTERNAL ERROR: Type mismatch"
            );

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
        } else if dst_id < src_id {
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

    src.data
        .delete_row(&mut world.entity_index, src_row, should_drop);

    world
        .entity_index
        .set_location(entity, NonNull::from(dst), dst_row);

    dst_row
}

pub(crate) fn move_entity_to_root(world: &mut World, entity: Entity) {
    let r = world.entity_index.get_record_mut(entity).unwrap();

    if r.table.is_none() {
        // SAFETY:
        // * root_table is NonNull.
        // * root_table should never contain columns, so only the entities array is initialized.
        r.table = Some(world.root_table);
        r.row = unsafe { world.root_table.as_mut().data.new_row_uninit(entity) };
    } else {
        // SAFETY: we just checked that the table is not None.
        let mut table = unsafe { r.table.unwrap_unchecked() };
        let mut root = world.root_table;
        let row = r.row;

        if table != world.root_table {
            // SAFETY:
            // - row is valid in enitity index.
            // - we just checked that src and dst tables are not the same.
            unsafe {
                move_entity(world, entity, table.as_mut(), row, root.as_mut());
            }
        }
    }
}
