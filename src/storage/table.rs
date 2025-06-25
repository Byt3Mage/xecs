use super::column::ColumnVec;
use crate::type_traits::{Component, Data};
use crate::{
    flags::TableFlags,
    graph::GraphNode,
    id::{Id, IdMap, Signature, id_index::IdLocation},
    table_index::TableId,
    type_traits::DataComponent,
    world::World,
};
use std::ptr::NonNull;

pub(crate) struct TableData {
    ids: Vec<Id>,
    columns: Box<[ColumnVec]>,
}

impl TableData {
    pub(crate) fn new(columns: Box<[ColumnVec]>) -> Self {
        Self {
            ids: vec![],
            columns,
        }
    }

    #[inline]
    pub(crate) fn ids(&self) -> &[Id] {
        &self.ids
    }

    #[inline]
    pub(crate) fn column(&self, index: usize) -> &ColumnVec {
        &self.columns[index]
    }

    /// Returns number of rows in this table.
    #[inline]
    pub(crate) fn row_count(&self) -> usize {
        self.ids.len()
    }

    /// Creates a new row without initializing its elements.
    /// This function will grow all columns if necessary.
    ///
    /// # Safety
    /// - The rows for the new id in all columns will be uninitialized (hence, unsafe).
    /// - The caller must ensure to write to all the columns in the new row.
    pub(crate) unsafe fn new_row(&mut self, id: Id) -> usize {
        let row = self.ids.len();
        self.ids.push(id);
        row
    }

    // TODO: docs
    pub(crate) unsafe fn push<T>(&mut self, col: usize, val: T) {
        debug_assert!(col < self.columns.len(), "column out of bounds");
        unsafe { self.columns.get_unchecked_mut(col).push(val) }
    }

    /// Returns a reference to the element at `row`, in `column`.
    ///
    /// This function does not perform bounds checking.
    ///
    /// # Safety
    /// - Caller ensures that `row` and `column` are valid.
    /// - Caller ensures that `T` is the value type of the column.
    pub(crate) unsafe fn get<T>(&self, col: usize, row: usize) -> &T {
        debug_assert!(col < self.columns.len(), "column out of bounds");
        unsafe { self.columns.get_unchecked(col).get(row) }
    }

    /// Returns a reference to the element at `row`, in `column`.
    ///
    /// This function does not perform bounds checking.
    ///
    /// # Safety
    /// - Caller ensures that `row` and `column` are valid.
    /// - Caller ensures that `T` is the value type of the column.
    pub(crate) unsafe fn get_mut<T: DataComponent>(&mut self, col: usize, row: usize) -> &mut T {
        debug_assert!(col < self.columns.len(), "column out of bounds");
        unsafe { self.columns.get_unchecked_mut(col).get_mut(row) }
    }

    /// Returns a pointer to the element at `row`, in `column`.
    ///
    /// This function does not perform bounds checking.
    ///
    /// # Safety
    /// - Caller ensures that `row` and `column` are valid.
    pub(crate) unsafe fn get_ptr(&self, col: usize, row: usize) -> NonNull<u8> {
        debug_assert!(col < self.columns.len(), "column out of bounds");
        // SAFETY: The caller ensures that `row` and `column` in bounds.
        unsafe { self.columns.get_unchecked(col).get_ptr(row) }
    }

    /// Returns a mutable reference to the element at `row`, in `column`.
    ///
    /// This function does not perform bounds checking.
    ///
    /// # Safety
    /// - The caller ensures that `row` and `column` are valid.
    pub(crate) unsafe fn get_ptr_mut(&mut self, col: usize, row: usize) -> NonNull<u8> {
        debug_assert!(col < self.columns.len(), "TableData: column out of bounds");
        // SAFETY: The caller ensures that `row` and `column` in bounds.
        unsafe { self.columns.get_unchecked_mut(col).get_ptr_mut(row) }
    }

    /// # Safety
    /// - `row` must be in bounds
    /// - `drop_check` must have the same length as `self.columns`
    pub(super) unsafe fn delete_row(&mut self, row: usize, drop_check: &[bool]) -> Option<Id> {
        debug_assert!(row < self.ids.len(), "TableData: row out of bounds");
        debug_assert!(drop_check.len() == self.columns.len());

        for (col, &should_drop) in self.columns.iter_mut().zip(drop_check) {
            unsafe {
                if should_drop {
                    col.swap_remove_drop(row);
                } else {
                    col.swap_remove(row);
                }
            }
        }

        let removed = self.ids.swap_remove(row);

        if row == self.ids.len() {
            None
        } else {
            Some(removed)
        }
    }
}

pub(crate) struct Table {
    /// Handle to self in [TableIndex](super::table_index::TableIndex).
    pub(crate) id: TableId,
    /// Flags describing the capabilites of this table
    pub(crate) _flags: TableFlags,
    /// Vector of component [Id] ids
    pub(crate) signature: Signature,
    /// Storage for ids and components.
    pub(crate) data: TableData,
    /// Maps column ids to columns indices.
    pub(crate) column_map: IdMap<usize>,
    /// Node representation for traversals.
    pub(crate) node: GraphNode,
}

impl Table {
    pub(crate) fn validate_data(&self) {
        #[cfg(debug_assertions)]
        {
            let len = self.data.row_count();

            self.data
                .columns
                .iter()
                .for_each(|col| assert_eq!(len, col.len()));
        }
    }

    /// Gets a reference to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be valid in this table.
    /// - `T` must be the value type of the column.
    #[inline]
    pub(crate) unsafe fn get<T: DataComponent>(&self, column_id: Id, row: usize) -> Option<&T> {
        self.column_map
            .get(column_id)
            .map(|&column| unsafe { self.data.get(column, row) })
    }

    /// Gets a reference to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be valid in this table.
    /// - `T` must be the value type of the column.
    #[inline]
    pub(crate) unsafe fn get_mut<T: DataComponent>(
        &mut self,
        row: usize,
        column_id: Id,
    ) -> Option<&mut T> {
        self.column_map
            .get(column_id)
            .map(|&col| unsafe { self.data.get_mut(col, row) })
    }

    /// Gets a ptr to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be valid in this table.
    #[inline]
    pub(crate) unsafe fn get_ptr(&self, row: usize, comp: Id) -> Option<NonNull<u8>> {
        // SAFETY:
        // - Column index is valid and immutable when we create the table.
        // - Caller ensures row is valid, which it must be if the entity we're getting for is valid.
        self.column_map
            .get(comp)
            .map(|&col| unsafe { self.data.get_ptr(col, row) })
    }

    /// Gets a mutable ptr to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be valid in this table.
    #[inline]
    pub(crate) unsafe fn get_ptr_mut(&mut self, row: usize, comp: Id) -> Option<NonNull<u8>> {
        // SAFETY:
        // - Column index is valid and immutable when we create the table.
        // - Caller ensures row is valid, which it must be if the entity we're getting for is valid.
        self.column_map
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

    debug_assert!(src_row < src.data.row_count(), "row out of bounds");

    // Append a new row to the destination table, but don't initialize columns.
    let dst_row = unsafe { dst.data.new_row(id) };
    let src_columns = &mut src.data.columns;
    let dst_columns = &mut dst.data.columns;
    let mut drop_check = vec![true; src_columns.len()];

    for (i_src, src_col) in src_columns.iter_mut().enumerate() {
        if let Some(&i_dst) = dst.column_map.get(src_col.id()) {
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
        world.id_index.set_location(
            i,
            IdLocation {
                table: src.id, // set table just to be pendatic, not really necessary.
                row: src_row,
            },
        );
    }

    // update record of moved entity.
    world.id_index.set_location(
        id,
        IdLocation {
            table: dst.id,
            row: dst_row,
        },
    );
}
