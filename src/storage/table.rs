use super::table_data::TableData;
use crate::{
    error::{EcsError, EcsResult},
    flags::TableFlags,
    graph::GraphNode,
    id::{Id, IdMap},
    pointer::{Ptr, PtrMut},
    table_index::TableId,
    types::IdList,
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
    /// Gets a ptr to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be a valid in this table.
    #[inline]
    pub(crate) unsafe fn get_ptr(&self, row: usize, id: Id) -> EcsResult<Ptr> {
        // SAFETY:
        // - Column index index is valid and immutable when we create the table.
        // - Caller ensures row is valid, which it must be if the entity we're getting for is valid.
        match self.component_map.get(id) {
            Some(&col) => Ok(unsafe { self.data.get_ptr(col, row) }),
            None => Err(EcsError::MissingComponent {
                entity: unsafe { *self.data.get_entity_unchecked(row) },
                comp: id,
            }),
        }
    }

    /// Gets a mutable ptr to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be a valid in this table.
    #[inline]
    pub(crate) unsafe fn get_ptr_mut(&mut self, row: usize, id: Id) -> EcsResult<PtrMut> {
        // SAFETY:
        // - Column index index is valid and immutable when we create the table.
        // - Caller ensures row is valid, which it must be if the entity we're getting for is valid.
        match self.component_map.get(id) {
            Some(&col) => Ok(unsafe { self.data.get_ptr_mut(col, row) }),
            None => Err(EcsError::MissingComponent {
                entity: unsafe { *self.data.get_entity_unchecked(row) },
                comp: id,
            }),
        }
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
    entity: Id,
    src: TableId,
    src_row: usize,
    dst: TableId,
) -> usize {
    let (src, dst) = world.table_index.get_2_mut(src, dst).unwrap();

    debug_assert!(src_row < src.data.entity_count(), "row out of bounds");

    // Append a new row to the destination table, but don't initialize columns.
    let dst_row = unsafe { dst.data.new_row(entity) };
    let src_columns = &mut src.data.columns;
    let dst_columns = &mut dst.data.columns;
    let mut drop_check = vec![true; src_columns.len()];

    for (i_src, src_col) in src_columns.iter_mut().enumerate() {
        if let Some(&i_dst) = dst.component_map.get(src_col.id()) {
            let dst_col = &mut dst_columns[i_dst];
            // SAFETY:
            // - We guarantee that src_row and dst_row are valid.
            // - We ensure that src_col and dst_col contain the same item type.
            unsafe { src_col.move_row_to(src_row, dst_col, dst_row) };
            drop_check[i_src] = false;
        } else {
            // Component not in destination table.
            // TODO: Emit remove hooks
        }
    }

    // update the record of the entity swapped into src_row.
    if let Some(e) = unsafe { src.data.delete_row(src_row, &drop_check) } {
        // unwrap should never fail, we don't keep invalid entities in tables.
        let r = world.id_index.get_record_mut(e).unwrap();
        r.table = src.id; // set table just to be pendatic, not really necessary.
        r.row = src_row;
    }

    // update record of moved entity.
    let r = world.id_index.get_record_mut(entity).unwrap();
    r.table = dst.id;
    r.row = dst_row;

    dst_row
}

pub(crate) fn move_entity_to_root(world: &mut World, entity: Id) {
    let r = world.id_index.get_record_mut(entity).unwrap();

    if r.table.is_null() {
        let root = &mut world.table_index[world.root_table];
        // SAFETY:
        // - root_table should never contain columns,
        //   so only the entities array is initialized.
        r.table = root.id;
        r.row = unsafe { root.data.new_row(entity) };
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
