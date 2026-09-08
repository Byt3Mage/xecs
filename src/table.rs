use std::ptr::NonNull;

use crate::{
    TypeMeta,
    bundle::{Bundle, InsertEdge, RemoveEdge},
    component::{id::ComponentId, registry::ComponentRegistry, signature::Signature},
    ecs::Ecs,
    id::{
        Id,
        allocator::{IdAllocator, IdRecord},
    },
    memory::{RawBlock, RowLayout},
    table_registry::{TableId, TableRegistry},
};

pub(crate) struct Column {
    data: RawBlock,
    layout: RowLayout,
}

impl Column {
    pub(crate) fn new(meta: &TypeMeta) -> Self {
        let layout = RowLayout::from_meta(meta);
        Self { data: RawBlock::new(layout), layout }
    }

    pub(crate) fn row_ptr(&self, row: u32) -> NonNull<u8> {
        self.data.row(self.layout.size(), row)
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
    unsafe fn drop(&mut self, len: u32, cap: u32) {
        unsafe {
            self.data.drop_rows(self.layout, 0..len);
            self.data.dealloc(self.layout, cap);
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
                unsafe { col.data.grow(col.layout, old, new) }
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
            self.columns.iter().for_each(|c| c.data.drop_row(c.layout, row));
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
            unsafe { col.data.shift(col.layout, last, row, 1) };
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
    values: Vec<u32>,
}

impl ColumnMap {
    pub(crate) fn new() -> Self {
        Self { values: vec![] }
    }

    pub(crate) fn insert(&mut self, id: ComponentId, value: u32) {
        let idx = id.index();
        if idx >= self.values.len() {
            self.values.resize(idx + 1, u32::MAX);
        }
        self.values[idx] = value;
    }

    pub(crate) fn try_get(&self, id: ComponentId) -> Option<u32> {
        self.values.get(id.index()).and_then(|&v| (v != u32::MAX).then_some(v))
    }

    pub(crate) fn contains(&self, id: ComponentId) -> bool {
        self.values.get(id.index()).is_some_and(|&v| v != u32::MAX)
    }
}

pub struct Table {
    pub(crate) sig: Signature,
    pub(crate) data: TableData,
    pub(crate) col_map: ColumnMap,
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
    pub(crate) fn column(&self, col: u32) -> &Column {
        &self.data.columns[col as usize]
    }

    #[inline(always)]
    pub(crate) fn column_ptr(&self, col: u32) -> NonNull<u8> {
        self.data.columns[col as usize].data.ptr()
    }
}

pub(crate) fn new_table(id: TableId, components: &mut ComponentRegistry, sig: Signature) -> Table {
    let mut col_map = ColumnMap::new();
    let mut columns = Vec::with_capacity(sig.len());

    for (i, &cid) in sig.iter().enumerate() {
        let ci = &mut components[cid];
        ci.insert_table(id);
        col_map.insert(cid, i as u32);
        columns.push(Column::new(&ci.meta))
    }

    Table { sig, col_map, data: TableData::new(columns.into()) }
}

/// Moves `id`'s row from `r.table` to `edge.dst` under `bundle`, leaving every
/// destination column initialized.
///
/// # Safety
/// - `id` is live at `r`, and `dst_row` was allocated in `edge.dst` for `id`
///   (or is `r.row` when `edge.dst` is `r.table`).
/// - `edge` was built by `plan` for this source table and this bundle.
/// - `bundle`'s fields are ascending by component id and each payload is a
///   valid value of its column's type; ownership transfers to the table.
/// - Component drop glue does not unwind.
pub(crate) unsafe fn insert(
    tables: &mut TableRegistry,
    ids: &mut IdAllocator,
    id: Id,
    r: IdRecord,
    edge: &InsertEdge,
    bundle: impl Bundle,
) {
    match edge {
        InsertEdge::InPlace { writes } => {
            let table = &tables[r.table];
            let mut i = 0;
            // SAFETY: `write` yields one payload per id, in `cols` order.
            unsafe {
                bundle.write(&mut |src| {
                    let col = table.column(writes[i].col);
                    let row = col.data.row(col.layout.size(), r.row);
                    col.layout.drop(row);
                    row.copy_from_nonoverlapping(src, col.layout.size());
                    i += 1;
                });
            }
        }
        InsertEdge::Moved { dst: dst_id, drops, moves, writes } => {
            let (src, dst) = tables.get_2_mut(r.table, *dst_id);
            // SAFETY: every destination column is filled below.
            let dst_row = unsafe { dst.data.alloc_row(id) };

            // Components a payload replaces. Dropped before the row is vacated so
            // the vacate itself has nothing left to release.
            for d in drops.iter() {
                let c = src.column(d.col);
                // SAFETY: `r.row` is live and initialized in this column.
                unsafe { c.data.drop_row(c.layout, r.row) };
            }

            for m in moves.iter() {
                let s = src.column(m.src);
                let d = dst.column(m.dst);
                // SAFETY: the pair comes from the plan, so both columns hold
                // the same component and therefore the same type; both rows
                // are valid, and the source slot is left dead.
                unsafe { s.data.move_row(s.layout, r.row, &d.data, dst_row, 1) };
            }

            let mut i = 0;
            // SAFETY: `write` yields one payload per id, in `cols` order.
            unsafe {
                bundle.write(&mut |src| {
                    let col = dst.column(writes[i].col);
                    let row = col.data.row(col.layout.size(), r.row);
                    col.layout.drop(row);
                    row.copy_from_nonoverlapping(src, col.layout.size());
                    i += 1;
                });
            }

            // SAFETY: every column of `r.row` has been moved out or dropped.
            if let Some(moved) = unsafe { src.data.swap_remove_row_no_drop(r.row) } {
                ids.set_location(moved, IdRecord { table: r.table, row: r.row });
            }

            ids.set_location(id, IdRecord { table: *dst_id, row: dst_row });
        }
    }
}

/// Moves `id`'s row from `r.table` to `edge.dst` under `bundle`, leaving every
/// destination column initialized.
///
/// # Safety
/// - `id` is live at `r`, and `dst_row` was allocated in `edge.dst` for `id`
///   (or is `r.row` when `edge.dst` is `r.table`).
/// - `edge` was built by `plan` for this source table and this bundle.
/// - `bundle`'s fields are ascending by component id and each payload is a
///   valid value of its column's type; ownership transfers to the table.
/// - Component drop glue does not unwind.
pub(crate) unsafe fn remove(tables: &mut TableRegistry, ids: &mut IdAllocator, id: Id, r: IdRecord, edge: &RemoveEdge) {
    let RemoveEdge::Moved { dst: dst_id, drops, moves } = edge else { return };

    let (src, dst) = tables.get_2_mut(r.table, *dst_id);

    // SAFETY: every destination column is filled by `moves` below. The
    // destination signature is the source's minus the dropped components.
    let dst_row = unsafe { dst.data.alloc_row(id) };

    // Components removed from `src`.
    for d in drops {
        let col = src.column(d.col);
        // SAFETY: The row comes from a live id record and
        // the data is no longer read from since the id is
        // moved across tables.
        unsafe { col.data.drop_row(col.layout, r.row) };
    }

    // Components transfered from `src` to `dst`.
    for mov in moves {
        let s = src.column(mov.src);
        let d = dst.column(mov.dst);
        // SAFETY: the pair comes from `plan`, so both columns hold the same
        // component and therefore the same type. Both rows are valid.
        unsafe { s.data.move_row(s.layout, r.row, &d.data, dst_row, 1) };
    }

    if let Some(moved) = unsafe { src.data.swap_remove_row_no_drop(r.row) } {
        ids.set_location(moved, r);
    }

    ids.set_location(id, IdRecord { table: *dst_id, row: dst_row });
}

pub(crate) unsafe fn remove_id(ecs: &mut Ecs, r: IdRecord) {
    if let Some(swapped) = unsafe { ecs.tables[r.table].data.swap_remove_row(r.row) } {
        ecs.ids.set_location(swapped, r);
    }
}
