use std::cmp::Ordering;

use crate::{
    ecs::Ecs,
    graph::GraphNode,
    id::{Id, Signature, map::IdMap},
    storage::{
        BorrowMutError, BorrowRefError,
        borrow::{BorrowMut, BorrowRef},
        column::Column,
    },
    table_index::TableId,
};

pub(crate) struct TableData {
    pub(crate) ids: Vec<Id>,
    pub(crate) columns: Box<[Column]>,
    capacity: usize,
}

impl TableData {
    pub(crate) fn new(columns: Box<[Column]>) -> Self {
        Self { ids: Vec::new(), columns, capacity: 0 }
    }

    #[inline(always)]
    pub(crate) fn num_rows(&self) -> usize {
        self.ids.len()
    }

    /// Ensure all columns have capacity for at least `additional` more rows.
    pub(crate) fn reserve(&mut self, additional: usize) {
        let required = self.num_rows().checked_add(additional).unwrap();

        if required <= self.capacity {
            return;
        }

        let old_cap = self.capacity;
        self.capacity = required;

        for col in self.columns.iter_mut() {
            unsafe { col.realloc(old_cap, required) };
        }

        self.ids.reserve(additional);
    }

    /// Append a row. Columns are uninitialized. Caller must write to every column.
    ///
    /// Returns the new row index.
    ///
    /// # Safety
    /// Caller must initialize all columns at the returned row before any read.
    pub(crate) unsafe fn alloc_row(&mut self, id: Id) -> usize {
        self.reserve(1);
        let row = self.num_rows();
        self.ids.push(id);
        row
    }

    /// Swap-remove a row. Moves the last row into the vacated slot.
    /// Returns the entity that was swapped in, if any.
    ///
    /// # Safety
    /// - `row` must be a valid row index.
    /// - Data at `row` for moved/dropped columns must already be handled by the caller.
    ///   This only moves the last row's data into the hole and truncates.
    pub(crate) unsafe fn swap_remove_row(&mut self, row: usize) -> Option<Id> {
        debug_assert!(row < self.num_rows());

        let last = self.num_rows() - 1;

        if row != last {
            for col in self.columns.iter() {
                unsafe { col.copy_row(last, row) };
            }
            let swapped = self.ids[last];
            self.ids[row] = swapped;
            self.ids.pop();
            Some(swapped)
        } else {
            self.ids.pop();
            None
        }
    }
}

impl Drop for TableData {
    fn drop(&mut self) {
        let len = self.num_rows();
        let cap = self.capacity;

        for col in self.columns.iter_mut() {
            unsafe { col.destroy(len, cap) }
        }

        self.capacity = 0;
        self.ids.clear();
    }
}

pub(crate) struct Table {
    /// Vector of component [Id]s
    pub(crate) sig: Signature,
    /// Ids stored on this table
    pub(crate) data: TableData,
    /// Maps ids to columns indices
    pub(crate) col_map: IdMap<usize>,
    /// Node representation for traversals
    pub(crate) graph_node: GraphNode,
}

impl Table {
    #[inline]
    pub fn num_rows(&self) -> usize {
        self.data.ids.len()
    }

    #[inline]
    pub fn num_cols(&self) -> usize {
        self.data.columns.len()
    }

    #[inline(always)]
    pub(crate) fn ids(&self) -> &[Id] {
        &self.data.ids
    }

    #[inline(always)]
    fn slice<'a, T: 'static>(&self, col: &'a Column) -> &'a mut [T] {
        col.assert_type::<T>();
        unsafe { core::slice::from_raw_parts_mut(col.as_ptr(), self.num_rows()) }
    }

    #[inline(always)]
    pub(crate) fn column_ref<T: 'static>(&self, col: usize) -> ColumnRef<'_, T> {
        let col = &self.data.columns[col];
        ColumnRef { borrow: col.borrow.borrow(), data: self.slice(col) }
    }

    #[inline(always)]
    pub(crate) fn column_mut<T: 'static>(&self, col: usize) -> ColumnMut<'_, T> {
        let col = &self.data.columns[col];
        ColumnMut {
            borrow: col.borrow.borrow_mut(),
            data: self.slice(col),
        }
    }

    #[inline(always)]
    pub(crate) fn try_column_ref<T: 'static>(&self, col: usize) -> Result<ColumnRef<'_, T>, BorrowRefError> {
        let col = &self.data.columns[col];
        col.borrow
            .try_borrow()
            .map(|b| ColumnRef { borrow: b, data: self.slice(col) })
    }

    #[inline(always)]
    pub(crate) fn try_column_mut<T: 'static>(&self, col: usize) -> Result<ColumnMut<'_, T>, BorrowMutError> {
        let col = &self.data.columns[col];
        col.borrow
            .try_borrow_mut()
            .map(|b| ColumnMut { borrow: b, data: self.slice(col) })
    }

    /// # Safety
    /// `row` must be a valid row index in this table.
    pub(crate) unsafe fn get<T: 'static>(&self, id: Id, row: usize) -> Option<&T> {
        unsafe { self.col_map.get(id).map(|&col| self.data.columns[col].get(row)) }
    }

    /// # Safety
    /// - `row` must be a valid row index in this table.
    pub(crate) unsafe fn get_mut<T: 'static>(&mut self, id: Id, row: usize) -> Option<&mut T> {
        unsafe { self.col_map.get(id).map(|&col| self.data.columns[col].get_mut(row)) }
    }
}

pub struct ColumnRef<'a, T> {
    data: &'a [T],
    #[allow(dead_code)] // held to release the borrow on drop
    borrow: BorrowRef<'a>,
}

pub struct ColumnMut<'a, T> {
    data: &'a mut [T],
    #[allow(dead_code)] // held to release the borrow on drop
    borrow: BorrowMut<'a>,
}

impl<T> std::ops::Deref for ColumnRef<'_, T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T> std::ops::Deref for ColumnMut<'_, T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T> std::ops::DerefMut for ColumnMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

/// Moves an entity from one table to another. Returns the new row for the id.
///
/// - For columns present in both tables, data is moved (memcpy + no drop on src).
/// - For columns only in src, data is dropped.
/// - For columns only in dst, the data is uninitialized and must be initialized
///   by the caller.
///
/// After the move, the src table's last row is swapped into the vacated row.
/// Entity records are updated for both the moved entity and the swapped entity.
///
/// # Safety
/// - `src_row` must be valid in src table.
/// - caller must initialize all uninitialized columns.
pub(crate) unsafe fn move_id(ecs: &mut Ecs, id: Id, src_table: TableId, src_row: usize, dst_table: TableId) -> usize {
    let [src, dst] = ecs.tables.get_2_mut(src_table, dst_table);

    // Allocate a new row in dst. Columns are uninitialized.
    let dst_row = unsafe { dst.data.alloc_row(id) };

    // Merge-walk both column arrays by component id.
    // Both are sorted by component id (same order as the signature)
    let mut si = 0;
    let mut di = 0;

    while si < src.num_cols() && di < dst.num_cols() {
        let src_col = &mut src.data.columns[si];
        let dst_col = &mut dst.data.columns[di];

        match src_col.id().cmp(&dst_col.id()) {
            Ordering::Equal => {
                // Column exists in both tables.
                // Move the raw bytes from src to dst.
                // The src slot will be overwritten by swap_remove
                // (or is last row and removed).
                unsafe {
                    let size = src_col.data_size();
                    let src_ptr = src_col.row_ptr(src_row, size);
                    let dst_ptr = dst_col.row_ptr(dst_row, size);
                    dst_ptr.copy_from_nonoverlapping(src_ptr, size);
                }
                si += 1;
                di += 1;
            }
            Ordering::Less => {
                // Column exists only in src, i.e., component is removed.
                // Drop the value.
                unsafe { src_col.drop_row(src_row) };
                si += 1;
            }
            Ordering::Greater => {
                // Column exists only in dst, i.e., component is added.
                // Caller initializes after move.
                di += 1;
            }
        }
    }

    // All remaining src columns are removed. Drop each.
    (si..src.num_cols()).for_each(|i| unsafe { src.data.columns[i].drop_row(src_row) });

    // We swap-remove the id from the src table.
    // Column data for shared/removed components at src_row has already
    // been moved out or dropped above, so swap_remove_row just moves
    // the last row's data into the hole.
    if let Some(swapped) = unsafe { src.data.swap_remove_row(src_row) } {
        ecs.ids.set_location(swapped, src_table, src_row);
    }

    // Update the moved id's record to point to dst.
    ecs.ids.set_location(id, dst_table, dst_row);
    dst_row
}
