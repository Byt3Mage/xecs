use super::{Column, TableData};
use crate::{
    component::ComponentLocation, flags::TableFlags, graph::GraphNode, id::HI_COMPONENT_ID,
    storage::table::Table, type_info::Type, world::World,
};
use std::{
    alloc::Layout,
    collections::HashMap,
    fmt::Display,
    mem::MaybeUninit,
    ptr::{self, NonNull},
    rc::Rc,
};

const PAGE_BITS: usize = 12;
const PAGE_SIZE: usize = 1 << PAGE_BITS;
const PAGE_MASK: usize = PAGE_SIZE - 1;

#[inline(always)]
const fn page_index(id: u32) -> usize {
    (id as usize) >> PAGE_BITS
}

#[inline(always)]
const fn page_offset(id: u32) -> usize {
    (id as usize) & PAGE_MASK
}

#[inline(always)]
const fn increase_version(id: TableId) -> TableId {
    TableId {
        idx: id.idx,
        ver: id.ver + 1,
    }
}

/// Stable, non-recycled handle into [TableIndex].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub(crate) struct TableId {
    idx: u32,
    ver: u32,
}

impl Display for TableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tableId({}, v{})", self.idx, self.ver)
    }
}

impl TableId {
    pub const fn null() -> Self {
        Self {
            idx: core::u32::MAX,
            ver: 1,
        }
    }
}

struct Page {
    sparse: Box<[usize; PAGE_SIZE]>,
    data: Box<[MaybeUninit<Table>; PAGE_SIZE]>,
}

impl Page {
    fn new() -> Self {
        let layout = Layout::array::<usize>(PAGE_SIZE).expect("Invalid layout");

        let sparse = unsafe {
            let ptr = std::alloc::alloc_zeroed(layout) as *mut usize;

            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout)
            }

            Box::from_raw(ptr as *mut [usize; PAGE_SIZE])
        };

        let data = unsafe { Box::new_uninit().assume_init() };
        Self { sparse, data }
    }
}

pub(crate) struct TableIndex {
    dense: Vec<TableId>,
    pages: Vec<Option<Page>>,
    alive_count: usize,
    max_id: u64,
}

impl Drop for TableIndex {
    fn drop(&mut self) {
        // Iterate through all alive entries
        // 0 is reserved for the root table
        for id in self.dense[1..self.alive_count].iter() {
            if let Some(page) = self
                .pages
                .get_mut(page_index(id.idx))
                .and_then(Option::as_mut)
            {
                let offset = page_offset(id.idx);
                let dense = page.sparse[offset];

                if dense != 0 && dense < self.alive_count && self.dense[dense] == *id {
                    // data is initialized, so it can be dropped safely
                    unsafe { page.data[offset].assume_init_drop() };
                }
            }
        }
    }
}

impl TableIndex {
    pub(crate) fn new() -> Self {
        Self {
            dense: Vec::new(),
            pages: Vec::new(),
            alive_count: 0,
            max_id: 0,
        }
    }

    fn ensure_page(pages: &mut Vec<Option<Page>>, idx: u32) -> &mut Page {
        let page_idx = page_index(idx);

        if page_idx >= pages.len() {
            pages.resize_with(page_idx + 1, || None);
        }
        // Allocate a new page if not already created
        pages[page_idx].get_or_insert_with(Page::new)
    }

    fn add_with_id<F>(&mut self, f: F) -> NonNull<Table>
    where
        F: FnOnce(TableId) -> Table,
    {
        let dense_count = self.dense.len();
        let alive_count = self.alive_count;

        debug_assert!(alive_count <= dense_count);

        if alive_count < dense_count {
            // recycle id.
            let id = self.dense[alive_count];
            let table = f(id);

            self.alive_count += 1;
            let page = Self::ensure_page(&mut self.pages, id.idx);
            let data = &mut page.data[page_offset(id.idx)];
            let ptr = data.as_mut_ptr();

            // SAFETY: ptr is stable and immovable. TODO
            unsafe {
                ptr.write(table);
                NonNull::new_unchecked(ptr)
            }
        } else {
            // create new id.
            let id = TableId {
                idx: (self.max_id + 1) as u32,
                ver: 0,
            };
            let table = f(id);

            self.max_id += 1;

            debug_assert!(
                self.max_id as u32 <= u32::MAX,
                "Max number of tables reached"
            );

            self.dense.push(id);

            let page = Self::ensure_page(&mut self.pages, id.idx);
            let offset = page_offset(id.idx);
            let data = &mut page.data[offset];
            let ptr = data.as_mut_ptr();

            page.sparse[offset] = self.alive_count;
            self.alive_count += 1;

            // SAFETY: ptr is stable. TODO
            unsafe {
                ptr.write(table);
                NonNull::new_unchecked(ptr)
            }
        }
    }

    /// Removes and returns the [Table] corresponding to the handle if it exists.
    fn remove(&mut self, id: TableId) -> Option<Table> {
        let page = self
            .pages
            .get_mut(page_index(id.idx))
            .and_then(Option::as_mut)?;
        let offset = page_offset(id.idx);
        let dense = page.sparse[offset];

        if dense == 0 || dense >= self.alive_count || self.dense[dense].ver != id.ver {
            return None;
        }

        let table = Some(unsafe { ptr::read(page.data[offset].as_mut_ptr()) });
        let last_alive = {
            self.alive_count -= 1;
            self.alive_count
        };
        let last_id = std::mem::replace(&mut self.dense[last_alive], increase_version(id));

        // swap last alive table with removed table.
        if dense != last_alive {
            let last_page = self
                .pages
                .get_mut(page_index(last_id.idx))
                .and_then(Option::as_mut)
                .expect("INTERNAL ERROR: Table index corrupted");

            last_page.sparse[page_offset(last_id.idx)] = dense;
            self.dense[dense] = last_id;
        }

        table
    }
}

pub(crate) struct TableBuilder {
    flags: TableFlags,
    type_: Type,
    node: GraphNode,
}

impl TableBuilder {
    pub(crate) fn new(type_ids: Type) -> Self {
        Self {
            flags: TableFlags::default(),
            type_: type_ids,
            node: todo!(),
        }
    }

    pub(crate) fn with_flags(mut self, flags: TableFlags) -> Self {
        self.flags |= flags;
        self
    }

    pub(crate) fn build(self, world: &mut World) -> NonNull<Table> {
        world.tables.add_with_id(|table_id| {
            let count = self.type_.id_count();
            let mut columns = Vec::with_capacity(count);
            let mut component_map_lo = [-1; HI_COMPONENT_ID as usize];
            let mut component_map_hi = HashMap::new();

            for (idx, &id) in self.type_.iter().enumerate() {
                let cr = world
                    .components
                    .get_mut(id)
                    .expect("Component record not found.");
                let mut cl = ComponentLocation {
                    id_index: idx,
                    id_count: 1,
                    column_index: -1,
                };

                // Component contains type_info, initialize a column for it.
                if let Some(ti) = &cr.type_info {
                    let col_idx = columns.len();

                    cl.column_index = col_idx as isize;

                    if id < HI_COMPONENT_ID {
                        component_map_lo[id as usize] = col_idx as isize;
                    } else {
                        component_map_hi.insert(id, col_idx);
                    }

                    columns.push(Column::new(id, Rc::clone(ti)));
                }

                cr.tables.insert(table_id, cl);
            }

            Table {
                id: table_id,
                flags: self.flags,
                type_: self.type_,
                component_map_lo,
                component_map_hi,
                node: self.node,
                data: TableData::new(columns.into()),
                traversable_count: 0,
            }
        })
    }
}
