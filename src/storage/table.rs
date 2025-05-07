use std::ptr;

use super::table_data::TableData;
use crate::{
    component::ComponentValue,
    entity::{Entity, EntityMap},
    flags::TableFlags,
    graph::GraphNode,
    table_index::TableId,
    types::IdList,
    world::World,
};

pub(crate) struct Table {
    /// Handle to self in [tableIndex](super::table_index::tableIndex).
    pub(crate) id: TableId,
    /// Flags describing the capabilites of this table
    pub(crate) flags: TableFlags,
    /// Vector of component [Entity] ids
    pub(crate) ids: IdList,
    /// Storage for entities and components.
    pub(crate) data: TableData,
    /// Maps component ids to columns.
    /// Uses specialized no-op hashing for faster operations.
    pub(crate) component_map: EntityMap<usize>,
    /// Node representation for traversals.
    pub(crate) node: GraphNode,
}

impl Table {
    /// Gets a reference to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be a valid in this table.
    /// - The type `C` must match the type of the column.
    #[inline]
    pub(crate) unsafe fn get<C: ComponentValue>(&self, row: usize, id: Entity) -> Option<&C> {
        debug_assert!(row < self.data.len(), "row out of bounds");
        // SAFETY:
        // - Column index index is valid and immutable when we create the table.
        // - Caller ensures row is valid,
        //   which it must be if the entity we're getting for is valid.
        self.component_map
            .get(&id)
            .map(|&c| unsafe { self.data.get_unchecked(c, row) })
    }

    /// Gets a mutable reference to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be a valid in this table.
    /// - The type `C` must match the type of the column.
    #[inline]
    pub(crate) unsafe fn get_mut<C: ComponentValue>(
        &mut self,
        row: usize,
        id: Entity,
    ) -> Option<&mut C> {
        // SAFETY:
        // - Column index index is valid and immutable when we create the table.
        // - Caller ensures row is valid,
        //   which it must be if the entity we're getting for is valid.
        self.component_map
            .get(&id)
            .map(|&c| unsafe { self.data.get_unchecked_mut(c, row) })
    }

    /// Sets the value of an initialized component of an entity.
    /// Essentially replaces the previously contained value.
    ///
    /// # Safety
    /// - `row` must be a valid in this table.
    /// - The type `C` must match the type of the column.
    #[inline]
    pub(crate) unsafe fn set<C: ComponentValue>(
        &self,
        row: usize,
        id: Entity,
        val: C,
    ) -> Option<()> {
        // SAFETY:
        // - Column index index is valid and immutable when we create the table.
        // - Caller ensures row is valid,
        //   which it must be if the entity we're getting for is valid.
        self.component_map
            .get(&id)
            .map(|&c| unsafe { self.data.set_unchecked(c, row, val) })
    }

    /// Sets the value of an uninitialized component of an entity.
    /// Should only be used to write the value of an uninit row after moving tables.
    ///
    /// # Safety
    /// - `row` must be a valid in this table.
    /// - The type `C` must match the type of the column.
    /// - The row must not have been initialized to avoid leaking memory.
    #[inline]
    pub(crate) unsafe fn set_uninit<C: ComponentValue>(
        &self,
        row: usize,
        id: Entity,
        val: C,
    ) -> Option<()> {
        // SAFETY:
        // - Column index index is valid and immutable when we create the table.
        // - Caller ensures row is valid,
        //   which it must be if the entity we're getting for is valid.
        // - Caller ensures that row is not initialized for column.
        self.component_map
            .get(&id)
            .map(|&c| unsafe { self.data.set_unitialized(c, row, val) })
    }
}

/// Moves entity from src table to dst.
/// Returns the row in dst table.
///
/// # Safety
/// - `src_row` must be a valid row in `src`.
/// - `src` and `dst` must not be the same table.
pub(crate) unsafe fn move_entity(
    world: &mut World,
    entity: Entity,
    src: TableId,
    src_row: usize,
    dst: TableId,
) -> usize {
    let (src, dst) = world.table_index.get_2_mut(src, dst).unwrap();

    debug_assert!(src_row < src.data.len(), "row out of bounds");

    // Append a new row to the destination table, but don't initialize columns.
    let dst_row = unsafe { dst.data.new_row_uninit(entity) };
    let src_columns = &mut src.data.columns;
    let dst_columns = &mut dst.data.columns;
    let mut drop_check = vec![true; src_columns.len()];

    for (i_src, src_col) in src_columns.iter_mut().enumerate() {
        if let Some(&i_dst) = dst.component_map.get(&src_col.id()) {
            let dst_col = &mut dst_columns[i_dst];
            let ti = &dst_col.type_info;

            debug_assert_eq!(ti.type_id, src_col.type_info.type_id, "type mismatch");

            let size = ti.size();

            // SAFETY:
            // - We guarantee that src_row and dst_row are valid.
            // - We ensure that src_col and dst_col contain the same type.
            // - Non-overlapping memory since both columns are different.
            unsafe {
                let src_data = src_col.data.as_ptr().add(src_row * size);
                let dst_data = dst_col.data.as_ptr().add(dst_row * size);
                ptr::copy_nonoverlapping(src_data, dst_data, size);
            }

            // Don't drop the data when deleting the row, since it's moved.
            drop_check[i_src] = false;
        } else {
            // Component not in destination table.
            // TODO:
            // Emit remove hooks
        }
    }

    // update the record of the entity swapped into src_row.
    if let Some(e) = unsafe { src.data.delete_row(src_row, &drop_check) } {
        // unwrap should never fail, we don't keep invalid entities in tables.
        let r = world.entity_index.get_record_mut(e).unwrap();
        r.table = src.id; // set table just to be pendatic, not really necessary.
        r.row = src_row;
    }

    // update record of moved entity.
    let r = world.entity_index.get_record_mut(entity).unwrap();
    r.table = dst.id;
    r.row = dst_row;

    dst_row
}

pub(crate) fn move_entity_to_root(world: &mut World, entity: Entity) {
    let r = world.entity_index.get_record_mut(entity).unwrap();

    if r.table.is_null() {
        let root = &mut world.table_index[world.root_table];
        // SAFETY:
        // - root_table should never contain columns,
        //   so only the entities array is initialized.
        r.table = root.id;
        r.row = unsafe { root.data.new_row_uninit(entity) };
    } else if r.table != world.root_table {
        let src = r.table;
        let row = r.row;

        // SAFETY:
        // - row is valid in enitity index.
        // - we just checked that src and dst tables are not the same.
        unsafe {
            move_entity(world, entity, src, row, world.root_table);
        }
    }
}
