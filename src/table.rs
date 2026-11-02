use std::ptr::NonNull;

use crate::{
    TypeMeta,
    bundle::{Bundle, InsertEdge, RemoveEdge},
    component::{ComponentId, registry::ComponentRegistry, signature::Signature},
    ecs::Ecs,
    id::{
        Id,
        allocator::{IdAllocator, IdRecord},
    },
    memory::{RawBlock, RowLayout},
    table_storage::{TableId, TableVec},
};

pub(crate) struct Column {
    data: RawBlock,
    layout: RowLayout,
}

impl Column {
    pub(crate) fn new(meta: &TypeMeta) -> Self {
        let layout = RowLayout::from_meta(meta);
        Self { data: layout.new_block(), layout }
    }

    #[inline(always)]
    pub(crate) fn row(&self, row: u32) -> NonNull<u8> {
        self.data.row(self.layout.size(), row)
    }

    /// Copy one value from `src` into `row`.
    ///
    /// # Safety
    /// `row` is within capacity, and `src` points to a valid value of this
    /// column's type.
    #[inline(always)]
    unsafe fn write_raw(&self, row: u32, src: NonNull<u8>) {
        unsafe { self.row(row).copy_from_nonoverlapping(src, self.layout.size()) };
    }

    /// Release the allocation.
    ///
    /// # Safety
    /// `cap` is the column's current capacity.
    #[inline]
    unsafe fn dealloc(&mut self, cap: u32) {
        unsafe { self.data.dealloc(self.layout, cap) };
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
    /// Column capacity always equals `ids.capacity()`.
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
    /// - The table has capacity for one more row (see [`reserve`](Self::reserve)).
    /// - Caller must initialize every column at the returned row before any read.
    pub(crate) unsafe fn push_row(&mut self, id: Id) -> u32 {
        let row = self.len();
        self.ids.push(id);
        row
    }

    /// Swap-remove `row`: the last row's values are copied into it and the
    /// table shrinks by one.
    ///
    /// Returns the id moved into `row`, if any.
    ///
    /// # Panics
    /// Panics if `row` is out of bounds.
    pub(crate) fn swap_remove_row(&mut self, row: u32) -> Option<Id> {
        self.ids.swap_remove(row as usize);
        let last = self.len();

        if row == last {
            return None;
        }

        for col in self.columns.iter_mut() {
            // SAFETY: `row < last`, and `last` was the final initialized row,
            // so both are within capacity.
            unsafe { col.data.swap_remove(col.layout.size(), row, last) };
        }

        Some(self.ids[row as usize])
    }
}

impl Drop for TableData {
    fn drop(&mut self) {
        let cap = self.ids.capacity() as u32;
        // SAFETY: `cap` is every column's capacity.
        self.columns.iter_mut().for_each(|c| unsafe { c.dealloc(cap) });
    }
}

pub struct Table {
    pub(crate) sig: Signature,
    pub(crate) data: TableData,
}

impl Table {
    pub(crate) fn create(id: TableId, components: &mut ComponentRegistry, sig: Signature) -> Self {
        let mut columns = Vec::with_capacity(sig.len());
        for &cid in sig.iter() {
            let ci = &mut components[cid];
            ci.insert_table(id);
            columns.push(Column::new(&ci.meta))
        }
        Self { sig, data: TableData::new(columns.into()) }
    }

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

    #[inline]
    pub(crate) fn has_component(&self, id: ComponentId) -> bool {
        self.sig.has_id(&id)
    }

    /// The column storing `id`, if this table has it.
    #[inline]
    pub(crate) fn column_of(&self, id: ComponentId) -> Option<&Column> {
        self.sig.find_id(&id).map(|i| &self.data.columns[i])
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

/// Writes `bundle` into `id`'s row, moving the row to `edge.dst` when that
/// is a different table.
///
/// # Safety
/// - `id` is live at `r`.
/// - `edge` was built for `r.table` and `bundle`'s set.
pub(crate) unsafe fn insert(
    tables: &mut TableVec<Table>,
    ids: &mut IdAllocator,
    id: Id,
    r: IdRecord,
    edge: &InsertEdge,
    bundle: impl Bundle,
) {
    if edge.dst == r.table {
        let table = &tables[r.table];
        let mut cols = edge.writes.iter();
        bundle.write(|src| {
            if let Some(&col) = cols.next() {
                // SAFETY:
                // - `r.row` is live.
                // - `edge.writes` holds one column per field in field
                //   order, so `src` is a value of `col`'s component.
                unsafe { table.column(col).write_raw(r.row, src) };
            }
        });
        return;
    }

    let (src, dst) = tables.get_2_mut(r.table, edge.dst);

    dst.data.reserve(1);

    // SAFETY: one row is reserved, and every destination column is written
    // below.
    let dst_row = unsafe { dst.data.push_row(id) };

    for m in edge.moves.iter() {
        let (s, d) = (src.column(m.src), dst.column(m.dst));
        // SAFETY:
        // - both columns hold the same component.
        // - `r.row` is live in `s`, and `dst_row` is within `d`'s capacity.
        unsafe { s.data.copy_rows(s.layout.size(), r.row, &d.data, dst_row, 1) };
    }

    let mut cols = edge.writes.iter();
    bundle.write(|p| {
        if let Some(&col) = cols.next() {
            // SAFETY:
            // - `dst_row` is within capacity.
            // - `edge.writes` holds one column per field in field order, so
            //   `p` is a value of `col`'s component.
            unsafe { dst.column(col).write_raw(dst_row, p) };
        }
    });

    if let Some(moved) = src.data.swap_remove_row(r.row) {
        ids.set_location(moved, r);
    }

    ids.set_location(id, IdRecord { table: edge.dst, row: dst_row });
}

/// Moves `id`'s row to `edge.dst`, carrying the components it keeps. Does
/// nothing when `edge.dst` is the current table.
///
/// # Safety
/// - `id` is live at `r`.
/// - `edge` was built for `r.table`.
pub(crate) unsafe fn remove(
    tables: &mut TableVec<Table>,
    ids: &mut IdAllocator,
    id: Id,
    r: IdRecord,
    edge: &RemoveEdge,
) {
    if edge.dst == r.table {
        return;
    }

    let (src, dst) = tables.get_2_mut(r.table, edge.dst);

    dst.data.reserve(1);

    // SAFETY: `edge.moves` covers every destination column and is applied below.
    let dst_row = unsafe { dst.data.push_row(id) };

    for m in edge.moves.iter() {
        let (s, d) = (src.column(m.src), dst.column(m.dst));
        // SAFETY:
        // - both columns hold the same component.
        // - `r.row` is live in `s`, and `dst_row` is within `d`'s capacity.
        unsafe { s.data.copy_rows(s.layout.size(), r.row, &d.data, dst_row, 1) };
    }

    if let Some(moved) = src.data.swap_remove_row(r.row) {
        ids.set_location(moved, r);
    }

    ids.set_location(id, IdRecord { table: edge.dst, row: dst_row });
}

/// Removes the row at `r`, moving the table's last row into its place.
///
/// # Panics
/// Panics if `r.row` is out of bounds for `r.table`.
pub(crate) fn remove_id(ecs: &mut Ecs, r: IdRecord) {
    if let Some(moved) = ecs.storage.tables[r.table].data.swap_remove_row(r.row) {
        ecs.ids.set_location(moved, r);
    }
}

/// Appends a row for `id` holding `bundle`. Returns the new row index.
///
/// # Safety
/// - `table` has capacity for one more row.
/// - `writes` holds one column per field of `bundle`'s set, in field order,
///   and together they cover every column of `table`.
pub(crate) unsafe fn create_row(table: &mut Table, id: Id, writes: &[u32], bundle: impl Bundle) -> u32 {
    // SAFETY: every column is written below, by `writes`.
    let row = unsafe { table.data.push_row(id) };
    let mut cols = writes.iter();
    bundle.write(|p| {
        if let Some(&col) = cols.next() {
            // SAFETY:
            // - `row` is within capacity.
            // - `writes` holds one column per field in field order, so `p`
            //   is a value of `col`'s component.
            unsafe { table.column(col).write_raw(row, p) };
        }
    });
    row
}
