use std::cmp::Ordering;

use crate::{
    ecs::Ecs,
    graph::GraphNode,
    id::{Id, Signature, map::IdMap},
    storage::column::Column,
    table_index::TableId,
};

pub(crate) struct TableData {
    ids: Vec<Id>,
    columns: Box<[Column]>,
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

    #[inline(always)]
    pub(crate) fn num_cols(&self) -> usize {
        self.columns.len()
    }

    #[inline(always)]
    pub(crate) fn ids(&self) -> &[Id] {
        &self.ids
    }

    #[inline(always)]
    pub(crate) fn columns(&self) -> &[Column] {
        &self.columns
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
            // SAFETY: required > old_cap (just checked); old_cap is current cap.
            unsafe { col.realloc(old_cap, required) };
        }
        self.ids.reserve(additional);
    }

    /// Append a row slot for `id`. Columns are uninitialized at the new row.
    ///
    /// Returns the new row index.
    ///
    /// # Safety
    /// Caller must initialize every column at the returned row before any read.
    pub(crate) unsafe fn alloc_row(&mut self, id: Id) -> usize {
        self.reserve(1);
        let row = self.num_rows();
        self.ids.push(id);
        row
    }

    /// Swap-remove the row at `row`: drops every column's value at `row`, then
    /// moves the last row's bytes into the hole and truncates.
    ///
    /// Returns the entity swapped into `row`, if any.
    ///
    /// # Safety
    /// `row` must be a valid row index. Unlike the previous design, this DROPS
    /// the row's data itself. Callers that have already moved a column's bytes
    /// out (e.g. table moves) must use [`swap_remove_row_no_drop`] instead.
    pub(crate) unsafe fn swap_remove_row(&mut self, row: usize) -> Option<Id> {
        debug_assert!(row < self.num_rows());
        // Drop the outgoing row's data in every column.
        for col in self.columns.iter() {
            // SAFETY: row is valid and initialized.
            unsafe { col.drop_row(row) };
        }
        unsafe { self.swap_remove_row_no_drop(row) }
    }

    /// Like [`swap_remove_row`] but assumes the row's data has ALREADY been
    /// dropped or moved out by the caller. Only relocates the last row.
    ///
    /// # Safety
    /// - `row` valid.
    /// - Every column's slot at `row` is already dropped or moved out.
    pub(crate) unsafe fn swap_remove_row_no_drop(&mut self, row: usize) -> Option<Id> {
        debug_assert!(row < self.num_rows());
        let last = self.num_rows() - 1;

        if row != last {
            for col in self.columns.iter() {
                // SAFETY: last and row both valid; copies last's bytes into hole.
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
    }
}

pub struct Table {
    pub(crate) sig: Signature,
    pub(crate) data: TableData,
    pub(crate) col_map: IdMap<usize>,
    pub(crate) graph_node: GraphNode,
}

impl Table {
    #[inline]
    pub fn num_rows(&self) -> usize {
        self.data.ids.len()
    }

    #[inline]
    pub fn num_cols(&self) -> usize {
        self.data.num_cols()
    }

    #[inline(always)]
    pub(crate) fn ids(&self) -> &[Id] {
        self.data.ids()
    }

    /// Borrow a column by index.
    #[inline(always)]
    pub(crate) fn column(&self, col: usize) -> &Column {
        &self.data.columns()[col]
    }

    /// # Safety
    /// - T matches column `col`'s type
    /// - No aliasing &mut for 'a.
    #[inline(always)]
    pub(crate) unsafe fn col_slice<T: 'static>(&self, col: usize) -> &[T] {
        unsafe { self.data.columns()[col].slice::<T>(self.num_rows()) }
    }

    /// # Safety
    /// - T matches column `col`'s type
    /// - Unique access for 'a.
    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    pub(crate) unsafe fn col_slice_mut<T: 'static>(&self, col: usize) -> &mut [T] {
        unsafe { self.data.columns()[col].slice_mut::<T>(self.num_rows()) }
    }

    /// Typed shared read of component `id` at `row`.
    ///
    /// # Safety
    /// - `row` valid in this table.
    /// - No `&mut` to the same element exists for the returned lifetime.
    pub(crate) unsafe fn get<T: 'static>(&self, id: Id, row: usize) -> Option<&T> {
        let col = self.col_map.get(id)?;
        // SAFETY: forwarded preconditions.
        Some(unsafe { self.data.columns()[*col].get(row) })
    }

    /// Typed exclusive read of component `id` at `row`.
    ///
    /// # Safety
    /// - `row` valid in this table.
    /// - No other borrow of the same element exists for the returned lifetime.
    pub(crate) unsafe fn get_mut<T: 'static>(&self, id: Id, row: usize) -> Option<&mut T> {
        let col = self.col_map.get(id)?;
        // SAFETY: forwarded preconditions.
        Some(unsafe { self.data.columns()[*col].get_mut(row) })
    }
}

/// Moves an entity from one table to another. Returns the new row for the id.
///
/// - For columns present in both tables, data is moved (memcpy + no drop on src).
/// - For columns only in src, data is dropped.
/// - For columns only in dst, the data is left uninitialized for caller to fill.
///
/// # Safety
/// - `src_row` must be valid in src table.
/// - caller must initialize all uninitialized columns.
pub(crate) unsafe fn move_id(ecs: &mut Ecs, id: Id, src_table: TableId, src_row: usize, dst_table: TableId) -> usize {
    let [src, dst] = ecs.tables.get_2_mut(src_table, dst_table);

    // SAFETY: caller initializes dst-only columns below / after.
    let dst_row = unsafe { dst.data.alloc_row(id) };

    // Merge-walk both column arrays by component id (both sorted by id).
    let mut si = 0;
    let mut di = 0;

    while si < src.num_cols() && di < dst.num_cols() {
        let src_col = src.column(si);
        let dst_col = dst.column(si);

        match src_col.id().cmp(&dst_col.id()) {
            Ordering::Equal => {
                // SAFETY: same component id ⇒ same type; rows valid.
                unsafe { src_col.move_row_to(src_row, dst_col, dst_row) };
                si += 1;
                di += 1;
            }
            Ordering::Less => {
                // Component removed: drop the src value.
                // SAFETY: src_row valid and initialized.
                unsafe { src_col.drop_row(src_row) };
                si += 1;
            }
            Ordering::Greater => {
                // Component added: dst slot stays uninitialized (caller fills).
                di += 1;
            }
        }
    }

    // Remaining src columns are removed components: drop each.
    for i in si..src.num_cols() {
        // SAFETY: src_row valid and initialized.
        unsafe { src.column(i).drop_row(src_row) };
    }

    // Src data at src_row is fully moved-out / dropped, so use the no-drop variant.
    // SAFETY: src_row valid; its columns are all moved out or dropped above.
    if let Some(swapped) = unsafe { src.data.swap_remove_row_no_drop(src_row) } {
        ecs.ids.set_location(swapped, src_table, src_row);
    }

    // Update the moved id's record to point to dst.
    ecs.ids.set_location(id, dst_table, dst_row);
    dst_row
}

pub(crate) unsafe fn remove_id(ecs: &mut Ecs, table: TableId, row: usize) {
    if let Some(swapped) = unsafe { ecs.tables[table].data.swap_remove_row(row) } {
        ecs.ids.set_location(swapped, table, row);
    }
}
