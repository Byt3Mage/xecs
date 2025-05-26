use super::table_data::TableData;
use crate::{
    component::Component,
    error::{EcsError, EcsResult, MissingComponent},
    flags::TableFlags,
    graph::GraphNode,
    id::{Id, IdList, IdMap},
    pointer::{Ptr, PtrMut},
    table_index::TableId,
    world::World,
};

pub(crate) struct Table {
    /// Handle to self in [tableIndex](super::table_index::tableIndex).
    pub(crate) id: TableId,
    /// Flags describing the capabilites of this table
    pub(crate) _flags: TableFlags,
    /// Vector of component [Entity] ids
    pub(crate) ids: IdList,
    /// Storage for entities and components.
    pub(crate) data: TableData,
    /// Maps component ids to columns.
    /// Uses specialized no-op hashing for faster operations.
    pub(crate) component_map: IdMap<usize>,
    /// Node representation for traversals.
    pub(crate) node: GraphNode,
}

impl Table {
    pub(crate) fn validate_data(&self) {
        #[cfg(debug_assertions)]
        {
            let len = self.data.entity_count();
            self.data
                .columns
                .iter()
                .for_each(|col| assert_eq!(len, col.len()));
        }
    }

    /// Sets the value of the component.
    /// Returns `Err(val)` if the table does not contain the component.
    ///
    /// # Safety
    /// - `row` must be a valid in this table.
    /// - `C` must be the same type as component.
    #[inline]
    pub(crate) unsafe fn try_replace<C: Component>(
        &mut self,
        row: usize,
        comp: Id,
        val: C,
    ) -> Result<C, C> {
        // SAFETY:
        // - Column index index is valid and immutable when we create the table.
        // - Caller ensures row is valid, which it must be if the entity we're getting for is valid.
        match self.component_map.get(comp) {
            Some(&col) => unsafe {
                let ptr = self.data.get_ptr_mut(row, col);
                Ok(std::mem::replace(ptr.as_mut::<C>(), val))
            },
            None => Err(val),
        }
    }

    /// Gets a ptr to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be valid in this table.
    #[inline]
    pub(crate) unsafe fn get_ptr(&self, row: usize, comp: Id) -> Option<Ptr> {
        // SAFETY:
        // - Column index index is valid and immutable when we create the table.
        // - Caller ensures row is valid, which it must be if the entity we're getting for is valid.
        self.component_map
            .get(comp)
            .map(|&col| unsafe { self.data.get_ptr(col, row) })
    }

    /// Gets a mutable ptr to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be valid in this table.
    #[inline]
    pub(crate) unsafe fn get_ptr_mut(&mut self, row: usize, comp: Id) -> Option<PtrMut> {
        // SAFETY:
        // - Column index index is valid and immutable when we create the table.
        // - Caller ensures row is valid, which it must be if the entity we're getting for is valid.
        self.component_map
            .get(comp)
            .map(|&col| unsafe { self.data.get_ptr_mut(col, row) })
    }
}

/// Moves `id` from src table to dst.
/// Returns the row in dst table.
///
/// # Safety
/// - `src_row` must be a valid row in `src`.
/// - `src` and `dst` must not be the same table.
pub(crate) unsafe fn move_id(
    world: &mut World,
    id: Id,
    src: TableId,
    src_row: usize,
    dst: TableId,
) {
    let (src, dst) = world.table_index.get_2_mut(src, dst).unwrap();

    debug_assert!(src_row < src.data.entity_count(), "row out of bounds");

    // Append a new row to the destination table, but don't initialize columns.
    let dst_row = unsafe { dst.data.new_row(id) };
    let src_columns = &mut src.data.columns;
    let dst_columns = &mut dst.data.columns;
    let mut drop_check = vec![true; src_columns.len()];

    for (i_src, src_col) in src_columns.iter_mut().enumerate() {
        if let Some(&i_dst) = dst.component_map.get(src_col.id()) {
            // SAFETY:
            // - We guarantee that src_row and dst_row are valid.
            // - We ensure that src_col and dst_col contain the same item type.
            unsafe { src_col.move_row_to(src_row, &mut dst_columns[i_dst]) };
            drop_check[i_src] = false;
        } else {
            // Component not in destination table.
            // TODO: Emit remove hooks
        }
    }

    // update the record of the id swapped into src_row.
    if let Some(i) = unsafe { src.data.delete_row(src_row, &drop_check) } {
        // unwrap should never fail, we don't keep invalid ids in tables.
        let r = world.id_index.get_record_mut(i).unwrap();
        r.table = src.id; // set table just to be pendatic, not really necessary.
        r.row = src_row;
    }

    // update record of moved entity.
    let r = world.id_index.get_record_mut(id).unwrap();
    r.table = dst.id;
    r.row = dst_row;
}
