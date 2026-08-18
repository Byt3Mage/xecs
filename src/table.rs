use std::{cmp::Ordering, ptr::NonNull};

use crate::{
    TypeMeta,
    component::{Signature, id::ComponentId},
    ecs::Ecs,
    graph::GraphNode,
    id::Id,
    memory::{RawBlock, RowMeta},
    table_index::TableId,
};

pub(crate) struct Column {
    id: ComponentId,
    meta: RowMeta,
    data: RawBlock,
}

impl Column {
    pub(crate) fn new(id: ComponentId, meta: &TypeMeta) -> Self {
        let meta = RowMeta::new(meta.layout, meta.drop);
        Self { id, meta, data: RawBlock::new(meta) }
    }

    pub(crate) fn row_ptr(&self, row: u32) -> NonNull<u8> {
        self.data.row(self.meta, row)
    }

    /// Drop all elements in the block and deallocate.
    ///
    /// The block is dead afterwards: `self.drop` is cleared, so a second
    /// call is a no-op rather than a double free. Values are dropped
    /// before the deallocation, so an unwinding element destructor
    /// leaks the allocation rather than freeing live memory.
    ///
    /// # Safety
    /// - `len` is the number of initialized elements.
    /// - `cap` is the current allocation capacity.
    pub(crate) unsafe fn drop(&mut self, len: u32, cap: u32) {
        unsafe {
            self.data.drop_rows(self.meta, 0..len);
            self.data.dealloc(self.meta, cap);
        }
    }
}

pub(crate) struct TableData {
    ids: Vec<Id>,
    columns: Box<[Column]>,
}

impl TableData {
    pub(crate) fn new(columns: Box<[Column]>) -> Self {
        Self { ids: vec![], columns }
    }

    #[inline(always)]
    pub(crate) fn len(&self) -> u32 {
        self.ids.len() as u32
    }

    #[inline(always)]
    pub(crate) fn num_cols(&self) -> usize {
        self.columns.len()
    }

    /// Ensure all columns have capacity for at least `additional` more rows.
    #[inline(always)]
    pub(crate) fn reserve(&mut self, additional: usize) {
        let old = self.ids.capacity() as u32;
        self.ids.reserve(additional);
        let new = self.ids.capacity() as u32;

        if new > old {
            for col in self.columns.iter_mut() {
                // SAFETY: new > old, and old is the current capacity.
                unsafe { col.data.grow(col.meta, old, new) }
            }
        }
    }

    /// Append a row slot for `id`. Columns are uninitialized at the new row.
    ///
    /// Returns the new row index.
    ///
    /// # Safety
    /// Caller must initialize every column at the returned row before any read.
    pub(crate) unsafe fn alloc_row(&mut self, id: Id) -> u32 {
        self.reserve(1);
        let row = self.len();
        self.ids.push(id);
        row
    }

    /// Swap-remove the row at `row`: drops every column's value at `row`,
    /// then moves the last row's bytes into the hole.
    ///
    /// Returns the entity swapped into `row`, if any.
    ///
    /// # Safety
    /// `row` must be a valid row index. This DROPS the row's data.
    /// Callers that have already moved a column's bytes out must use
    /// [`swap_remove_row_no_drop`].
    pub(crate) unsafe fn swap_remove_row(&mut self, row: u32) -> Option<Id> {
        debug_assert!(row < self.len());
        unsafe {
            // SAFETY: row is valid and initialized.
            self.columns.iter().for_each(|c| c.data.drop_row(c.meta, row));
            self.swap_remove_row_no_drop(row)
        }
    }

    /// Like [`swap_remove_row`] but assumes the row's data has ALREADY been
    /// dropped or moved out.
    ///
    /// # Safety
    /// - `row` valid.
    /// - Every column's slot at `row` is already dropped or moved out.
    pub(crate) unsafe fn swap_remove_row_no_drop(&mut self, row: u32) -> Option<Id> {
        debug_assert!(row < self.len());
        let last = self.len() - 1;

        self.ids.swap_remove(row as usize);

        if row == last {
            return None;
        }

        for col in self.columns.iter_mut() {
            unsafe { col.data.shift(col.meta, last, row, 1) };
        }

        Some(self.ids[row as usize])
    }
}

impl Drop for TableData {
    fn drop(&mut self) {
        let len = self.ids.len() as u32;
        let cap = self.ids.capacity() as u32;
        // SAFETY: rows `0..len` are initialized; `cap` is the columns'
        // capacity. The id vector releases itself.
        self.columns.iter_mut().for_each(|c| unsafe { c.drop(len, cap) });
    }
}

pub(crate) struct ColumnMap {
    values: Vec<usize>,
}

impl ColumnMap {
    pub(crate) fn new() -> Self {
        Self { values: vec![] }
    }

    pub(crate) fn insert(&mut self, id: ComponentId, value: usize) {
        let idx = id.index();
        if idx >= self.values.len() {
            self.values.resize(idx + 1, usize::MAX);
        }
        self.values[idx] = value;
    }

    pub(crate) fn get(&self, id: ComponentId) -> Option<usize> {
        self.values
            .get(id.index())
            .and_then(|&v| (v != usize::MAX).then_some(v))
    }

    pub(crate) fn contains(&self, id: ComponentId) -> bool {
        self.values.get(id.index()).is_some_and(|&v| v != usize::MAX)
    }
}

pub struct Table {
    pub(crate) sig: Signature,
    pub(crate) data: TableData,
    pub(crate) col_map: ColumnMap,
    pub(crate) node: GraphNode,
}

impl Table {
    #[inline]
    pub fn num_rows(&self) -> u32 {
        self.data.len()
    }

    #[inline]
    pub fn num_cols(&self) -> usize {
        self.data.num_cols()
    }

    #[inline(always)]
    pub(crate) fn ids(&self) -> &[Id] {
        &self.data.ids
    }

    /// Borrow a column by index.
    #[inline(always)]
    pub(crate) fn column(&self, col: usize) -> &Column {
        &self.data.columns[col]
    }

    #[inline(always)]
    pub(crate) fn column_ptr(&self, col: usize) -> NonNull<u8> {
        self.data.columns[col].data.ptr()
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
pub(crate) unsafe fn move_id(ecs: &mut Ecs, id: Id, src_table: TableId, src_row: u32, dst_table: TableId) -> u32 {
    let [src, dst] = ecs.tables.get_2_mut(src_table, dst_table);

    // SAFETY: caller initializes dst-only columns below / after.
    let dst_row = unsafe { dst.data.alloc_row(id) };

    // Merge-walk both column arrays by component id (both sorted by id).
    let mut si = 0;
    let mut di = 0;

    while si < src.num_cols() && di < dst.num_cols() {
        let src = src.column(si);
        let dst = dst.column(di);

        match src.id.cmp(&dst.id) {
            Ordering::Equal => {
                // SAFETY: same component id ⇒ same type; rows valid.
                unsafe { src.data.move_row(src.meta, src_row, &dst.data, dst_row, 1) };
                si += 1;
                di += 1;
            }
            Ordering::Less => {
                // Component removed: drop the src value.
                // SAFETY: src_row valid and initialized.
                unsafe { src.data.drop_row(src.meta, src_row) };
                si += 1;
            }
            Ordering::Greater => {
                // Component added: dst slot stays uninitialized (caller fills).
                di += 1;
            }
        }
    }

    // Remaining src columns are removed components, drop all.
    // SAFETY: src_row valid and initialized.
    for col in &mut src.data.columns[si..] {
        unsafe { col.data.drop_row(col.meta, src_row) };
    }

    // Src data at src_row is fully moved-out or dropped, so use the no-drop variant.
    // SAFETY: src_row valid; its columns are all moved out or dropped above.
    if let Some(swapped) = unsafe { src.data.swap_remove_row_no_drop(src_row) } {
        ecs.ids.set_location(swapped, src_table, src_row);
    }

    // Update the moved id's record to point to new table and row.
    ecs.ids.set_location(id, dst_table, dst_row);
    dst_row
}

pub(crate) unsafe fn remove_id(ecs: &mut Ecs, table: TableId, row: u32) {
    if let Some(swapped) = unsafe { ecs.tables[table].data.swap_remove_row(row) } {
        ecs.ids.set_location(swapped, table, row);
    }
}
