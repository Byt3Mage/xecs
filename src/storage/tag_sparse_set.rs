use std::{alloc::Layout, ptr};

use crate::entity::Entity;

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

type Page = Box<[usize; PAGE_SIZE]>;

fn new_page() -> Page {
    unsafe {
        let layout = Layout::array::<usize>(PAGE_SIZE).expect("Invalid layout");
        let ptr = std::alloc::alloc(layout);

        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout)
        }

        // set all bytes to 1, which means usize::MAX
        ptr::write_bytes(ptr, 0xFF, const { PAGE_SIZE * size_of::<usize>() });

        // SAFETY: the pointer is valid and aligned
        Box::from_raw(ptr as *mut [usize; PAGE_SIZE])
    }
}

/// Specialized sparse set of entities with associated data.
pub(crate) struct TagSparseSet {
    dense: Vec<Entity>,
    pages: Vec<Option<Page>>,
}

impl TagSparseSet {
    pub(crate) fn new() -> Self {
        Self {
            dense: vec![],
            pages: vec![],
        }
    }

    fn ensure_page(pages: &mut Vec<Option<Page>>, idx: u32) -> &mut Page {
        let page_idx = page_index(idx);

        if page_idx >= pages.len() {
            pages.resize_with(page_idx + 1, || None);
        }

        // Allocate a new page if not already created
        pages[page_idx].get_or_insert_with(new_page)
    }

    /// Inserts a value into the set for the given entity.
    /// Returns true if the entity was not already in the set.
    pub(crate) fn insert(&mut self, entity: Entity) -> bool {
        let page = Self::ensure_page(&mut self.pages, entity.id());
        let offset = page_offset(entity.id());
        let dense = page[offset];

        if dense == usize::MAX {
            page[offset] = self.dense.len();
            self.dense.push(entity);
            true
        } else {
            false
        }
    }

    /// Removes an entity from the set.
    /// Returns the true if the entity was in the set.
    pub(crate) fn remove(&mut self, entity: Entity) -> bool {
        let Some(page) = self
            .pages
            .get_mut(page_index(entity.id()))
            .and_then(Option::as_mut)
        else {
            return false;
        };

        let offset = page_offset(entity.id());
        let dense = page[offset];

        // entity not in set.
        if dense == usize::MAX || self.dense[dense] != entity {
            return false;
        }

        page[offset] = usize::MAX;

        let last_index = self.dense.len() - 1;

        self.dense.swap_remove(dense);

        if dense != last_index {
            let entity = self.dense[dense];
            let page = self.pages[page_index(entity.id())]
                .as_mut()
                .expect("INTERNAL ERROR: Sparse set corrupted");

            page[page_offset(entity.id())] = dense;
        }

        true
    }

    pub(crate) fn has_entity(&self, entity: Entity) -> bool {
        let Some(page) = self
            .pages
            .get(page_index(entity.id()))
            .and_then(Option::as_ref)
        else {
            return false;
        };

        page[page_offset(entity.id())] < self.dense.len()
    }
}
