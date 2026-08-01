use std::{
    alloc::{self, Layout, LayoutError},
    cmp::Ordering,
    ptr::NonNull,
};

use ahash::AHashMap;

use crate::{
    component::{Signature, id::ComponentId},
    ecs::Ecs,
    graph::GraphNode,
    id::Id,
    storage::block::Block,
    table_index::TableId,
};

/// [Id] must not require drop, i.e., must be a plain identifier
const _: () = const { assert!(!std::mem::needs_drop::<Id>()) };

#[inline(always)]
const fn ids_layout(cap: u32) -> Result<Layout, LayoutError> {
    const SIZE: usize = std::mem::size_of::<Id>();
    const ALIGN: usize = std::mem::align_of::<Id>();
    Layout::from_size_align(cap as usize * SIZE, ALIGN)
}

pub(crate) struct Column {
    pub(crate) id: ComponentId,
    pub(crate) data: Block,
}

pub(crate) struct TableData {
    columns: Box<[Column]>,
    ids: NonNull<Id>,
    len: u32,
    cap: u32,
}

impl TableData {
    pub(crate) fn new(columns: Box<[Column]>) -> Self {
        Self { ids: NonNull::dangling(), columns, len: 0, cap: 0 }
    }

    #[inline(always)]
    pub(crate) fn num_rows(&self) -> u32 {
        self.len
    }

    #[inline(always)]
    pub(crate) fn num_cols(&self) -> usize {
        self.columns.len()
    }

    /// Ensure all columns have capacity for at least `additional` more rows.
    #[inline(always)]
    pub(crate) fn reserve(&mut self, additional: u32) {
        let cap = self.cap;
        let req = self.len.checked_add(additional).unwrap();

        if req > cap {
            unsafe {
                let new = ids_layout(req).unwrap();
                let ptr = match cap == 0 {
                    true => std::alloc::alloc(new),
                    false => alloc::realloc(self.ids.as_ptr().cast(), ids_layout(cap).unwrap(), new.size()),
                };

                self.ids = match NonNull::new(ptr) {
                    Some(ptr) => ptr.cast(),
                    None => alloc::handle_alloc_error(new),
                };

                for col in self.columns.iter_mut() {
                    // SAFETY: required > capacity; old_cap is current capacity.
                    col.data.realloc(cap as usize, req as usize)
                }
            }
            self.cap = req;
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
        let row = self.len;
        self.len += 1;
        unsafe { self.ids.add(row as usize).write(id) };
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
    pub(crate) unsafe fn swap_remove_row(&mut self, row: u32) -> Option<Id> {
        debug_assert!(row < self.num_rows());
        // Drop the outgoing row's data in every column.
        for col in self.columns.iter() {
            // SAFETY: row is valid and initialized.
            unsafe { col.data.drop_row(row) };
        }
        unsafe { self.swap_remove_row_no_drop(row) }
    }

    /// Like [`swap_remove_row`] but assumes the row's data has ALREADY been
    /// dropped or moved out by the caller. Only relocates the last row.
    ///
    /// # Safety
    /// - `row` valid.
    /// - Every column's slot at `row` is already dropped or moved out.
    pub(crate) unsafe fn swap_remove_row_no_drop(&mut self, row: u32) -> Option<Id> {
        debug_assert!(row < self.num_rows());
        let last = self.num_rows() - 1;

        if row != last {
            unsafe {
                let src = self.ids.add(last as usize);
                let dst = self.ids.add(row as usize);
                let swapped = src.read();

                src.copy_to_nonoverlapping(dst, 1);
                self.columns.iter().for_each(|col| col.data.copy_row(last, row));
                self.len -= 1;
                Some(swapped)
            }
        } else {
            self.len -= 1;
            None
        }
    }
}

impl Drop for TableData {
    fn drop(&mut self) {
        if self.cap == 0 {
            return;
        }

        unsafe {
            let layout = ids_layout(self.cap).unwrap();
            alloc::dealloc(self.ids.as_ptr().cast(), layout);

            for col in self.columns.iter_mut() {
                col.data.destroy(self.len, self.cap)
            }
        }
    }
}

pub struct Table {
    pub(crate) sig: Signature,
    pub(crate) data: TableData,
    pub(crate) col_map: AHashMap<ComponentId, usize>,
    pub(crate) graph_node: GraphNode,
}

impl Table {
    #[inline]
    pub fn num_rows(&self) -> u32 {
        self.data.num_rows()
    }

    #[inline]
    pub fn num_cols(&self) -> usize {
        self.data.num_cols()
    }

    #[inline(always)]
    pub(crate) fn ids(&self) -> NonNull<Id> {
        self.data.ids
    }

    /// Borrow a column by index.
    #[inline(always)]
    pub(crate) fn column(&self, col: usize) -> &Column {
        &self.data.columns[col]
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
        let src_col = src.column(si);
        let dst_col = dst.column(di);

        match src_col.id.cmp(&dst_col.id) {
            Ordering::Equal => {
                // SAFETY: same component id ⇒ same type; rows valid.
                unsafe { src_col.data.move_row_to(src_row, &dst_col.data, dst_row) };
                si += 1;
                di += 1;
            }
            Ordering::Less => {
                // Component removed: drop the src value.
                // SAFETY: src_row valid and initialized.
                unsafe { src_col.data.drop_row(src_row) };
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
        unsafe { src.column(i).data.drop_row(src_row) };
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

pub(crate) unsafe fn remove_id(ecs: &mut Ecs, table: TableId, row: u32) {
    if let Some(swapped) = unsafe { ecs.tables[table].data.swap_remove_row(row) } {
        ecs.ids.set_location(swapped, table, row);
    }
}
