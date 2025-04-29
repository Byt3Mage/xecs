use crate::{component::ComponentValue, entity::Entity};
use std::{
    alloc::Layout,
    ptr::{self, NonNull},
    usize,
};

type Page<const N: usize> = Box<[usize; N]>;

fn new_page<const SIZE: usize>() -> Page<SIZE> {
    unsafe {
        let layout = Layout::array::<usize>(SIZE).expect("Invalid layout");
        let ptr = std::alloc::alloc(layout);

        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout)
        }

        // set all bytes to 1, which means usize::MAX
        ptr::write_bytes(ptr, 0xFF, const { SIZE * size_of::<usize>() });

        // SAFETY: the pointer is valid and aligned
        Box::from_raw(ptr as *mut [usize; SIZE])
    }
}

struct Entry<T> {
    value: T,
    entity: Entity,
}

/// Specialized sparse set of entities with associated data.
pub(crate) struct PagedSparseSet<T, const PAGE_SIZE: usize> {
    dense: Vec<Entry<T>>,
    pages: Vec<Option<Page<PAGE_SIZE>>>,
}

impl<T, const PAGE_SIZE: usize> PagedSparseSet<T, PAGE_SIZE> {
    const PAGE_BITS: usize = PAGE_SIZE.trailing_zeros() as usize;
    const PAGE_MASK: usize = PAGE_SIZE - 1;

    #[inline(always)]
    const fn page_index(id: u32) -> usize {
        (id as usize) >> Self::PAGE_BITS
    }

    #[inline(always)]
    const fn page_offset(id: u32) -> usize {
        (id as usize) & Self::PAGE_MASK
    }

    pub(crate) fn new() -> Self {
        Self {
            dense: vec![],
            pages: vec![],
        }
    }

    fn ensure_page(pages: &mut Vec<Option<Page<PAGE_SIZE>>>, idx: u32) -> &mut Page<PAGE_SIZE> {
        let page_idx = Self::page_index(idx);

        if page_idx >= pages.len() {
            pages.resize_with(page_idx + 1, || None);
        }

        // Allocate a new page if not already created
        pages[page_idx].get_or_insert_with(new_page)
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data and returns the old value if the entity is already in the set.
    pub(crate) fn insert(&mut self, entity: Entity, value: T) -> Option<T> {
        debug_assert!(!entity.is_null(), "can't insert null entity into set");

        let page = Self::ensure_page(&mut self.pages, entity.id());
        let offset = Self::page_offset(entity.id());
        let dense = page[offset];

        if dense != usize::MAX {
            debug_assert!(dense < self.dense.len(), "dense index out of bounds");

            let entry = &mut self.dense[dense];

            if entry.entity == entity {
                Some(std::mem::replace(&mut entry.value, value))
            } else {
                None
            }
        } else {
            page[offset] = self.dense.len();
            self.dense.push(Entry { entity, value });
            None
        }
    }

    /// Removes an entity from the set.
    /// Returns the value associated with the entity if it was present.
    pub(crate) fn remove(&mut self, entity: Entity) -> Option<T> {
        let page = self
            .pages
            .get_mut(Self::page_index(entity.id()))
            .and_then(Option::as_mut)?;
        let offset = Self::page_offset(entity.id());
        let dense = page[offset];

        // entity not in set.
        if dense == usize::MAX || self.dense[dense].entity != entity {
            return None;
        }

        page[offset] = usize::MAX;

        let last_index = self.dense.len() - 1;
        let removed = self.dense.swap_remove(dense);

        if dense != last_index {
            let entity = &self.dense[dense].entity;
            let page = self.pages[Self::page_index(entity.id())]
                .as_mut()
                .expect("Sparse set corrupted");
            page[Self::page_offset(entity.id())] = dense;
        }

        Some(removed.value)
    }

    pub(crate) fn has_entity(&self, entity: Entity) -> bool {
        self.pages
            .get(Self::page_index(entity.id()))
            .and_then(Option::as_ref)
            .is_some_and(|page| page[Self::page_offset(entity.id())] != usize::MAX)
    }

    pub(crate) fn get(&self, entity: Entity) -> Option<&T> {
        let page = self
            .pages
            .get(Self::page_index(entity.id()))
            .and_then(Option::as_ref)?;
        let dense = page[Self::page_offset(entity.id())];

        if dense == usize::MAX {
            return None;
        }

        debug_assert!(dense < self.dense.len(), "dense index out of bounds");

        let entry = &self.dense[dense];

        if entry.entity == entity {
            Some(&entry.value)
        } else {
            None
        }
    }

    pub(crate) fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let page = self
            .pages
            .get(Self::page_index(entity.id()))
            .and_then(Option::as_ref)?;
        let dense = page[Self::page_offset(entity.id())];

        if dense == usize::MAX {
            return None;
        }

        debug_assert!(dense < self.dense.len(), "dense index out of bounds");

        let entry = &mut self.dense[dense];

        if entry.entity == entity {
            Some(&mut entry.value)
        } else {
            None
        }
    }
}

#[inline(always)]
const fn id_to_sparse(entity: Entity) -> usize {
    entity.id() as usize
}

pub(crate) struct SparseSet<T> {
    dense: Vec<Entry<T>>,
    sparse: Vec<usize>,
}

impl<T> SparseSet<T> {
    pub(crate) fn new() -> Self {
        Self {
            dense: vec![],
            sparse: vec![],
        }
    }

    /// Resizes the sparse array such that
    /// it can hold at least (`index` + 1) entries.
    #[inline(always)]
    fn ensure_index(&mut self, index: usize) {
        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, usize::MAX);
        }
    }

    /// Inserts a value into the set for the given entity.
    /// Replaces the data and returns the old value if the entity is already in the set.
    pub(crate) fn insert(&mut self, entity: Entity, value: T) -> Option<T> {
        debug_assert!(!entity.is_null(), "can't insert null entity into set");

        let sparse = id_to_sparse(entity);
        self.ensure_index(sparse);

        let dense = self.sparse[sparse];

        if dense != usize::MAX {
            debug_assert!(dense < self.dense.len(), "dense index is out of bounds");

            /* Allowed to panic if the dense index is out of bounds.
             * This is because valid dense index must be within bounds of dense.
             */
            let entry = &mut self.dense[dense];

            if entry.entity == entity {
                Some(std::mem::replace(&mut entry.value, value))
            } else {
                None
            }
        } else {
            self.sparse[sparse] = self.dense.len();
            self.dense.push(Entry { entity, value });
            None
        }
    }

    /// Removes an entity from the set.
    /// Returns the value associated with the entity if it was present.
    pub(crate) fn remove(&mut self, entity: Entity) -> Option<T> {
        let sparse = id_to_sparse(entity);
        let dense = self.sparse.get(sparse).copied()?;

        // entity not in set.
        if dense == usize::MAX || self.dense[dense].entity != entity {
            return None;
        }

        self.sparse[sparse] = usize::MAX;

        let last_index = self.dense.len() - 1;
        let removed = self.dense.swap_remove(dense);

        if dense != last_index {
            let entity = self.dense[dense].entity;
            self.sparse[id_to_sparse(entity)] = dense;
        }

        Some(removed.value)
    }

    pub(crate) fn has_entity(&self, entity: Entity) -> bool {
        self.sparse
            .get(id_to_sparse(entity))
            .is_some_and(|sparse| *sparse != usize::MAX)
    }

    pub(crate) fn get(&self, entity: Entity) -> Option<&T> {
        let dense = *self.sparse.get(id_to_sparse(entity))?;

        if dense == usize::MAX {
            return None;
        }

        debug_assert!(dense < self.dense.len(), "dense index is out of bounds");

        let entry = &self.dense[dense];

        if entry.entity == entity {
            Some(&entry.value)
        } else {
            None
        }
    }

    pub(crate) fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let dense = *self.sparse.get(id_to_sparse(entity))?;

        if dense == usize::MAX {
            return None;
        }

        debug_assert!(dense < self.dense.len(), "dense index is out of bounds");

        let entry = &mut self.dense[dense];

        if entry.entity == entity {
            Some(&mut entry.value)
        } else {
            None
        }
    }
}
