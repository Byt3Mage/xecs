use const_assert::const_assert;
use std::{
    alloc::Layout,
    ptr::{self, NonNull},
    rc::Rc,
};

use crate::{
    component::ComponentValue,
    entity::Entity,
    pointer::{OwningPtr, PtrMut},
    storage::swap_entities,
    type_info::TypeInfo,
};

use super::{Column, TypeErased};

struct Dense {
    entities: NonNull<Entity>,
    column: Column,
    len: usize,
    cap: usize,
}

impl Drop for Dense {
    fn drop(&mut self) {
        if self.cap == 0 {
            return;
        }

        const_assert!(
            || std::mem::needs_drop::<Entity>() == false,
            "Entity type must not require drop, fix drop otherwise"
        );

        unsafe {
            let entt_layout = Layout::array::<Entity>(self.cap).expect("Invalid layout");
            let entt_ptr = self.entities.as_ptr() as *mut u8;
            std::alloc::dealloc(entt_ptr, entt_layout);

            let col = &self.column;
            let (size, align) = col.type_info.size_align();
            let layout = Layout::from_size_align(self.cap * size, align).expect("Invalid layout");

            if let Some(drop_fn) = col.type_info.hooks.drop_fn {
                let mut ptr = col.data;
                for _ in 0..self.len {
                    drop_fn(ptr);
                    ptr = ptr.add(size)
                }
            }

            std::alloc::dealloc(col.data.as_ptr(), layout);
        }
    }
}

impl Dense {
    fn new(component: Entity, type_info: Rc<TypeInfo>) -> Self {
        Self {
            entities: NonNull::dangling(),
            column: Column::new(component, type_info),
            len: 0,
            cap: 0,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    fn reserve(&mut self, additional: usize) {
        let required_cap = self.len.checked_add(additional).expect("capacity overflow");

        if required_cap <= self.cap {
            return;
        }

        unsafe {
            if self.cap == 0 {
                self.entities.alloc(required_cap);
                self.column.alloc(required_cap);
            } else {
                self.entities.realloc(self.cap, required_cap);
                self.column.realloc(self.cap, required_cap);
            };
        }

        self.cap = required_cap;
    }

    /// Adds the entity to the dense array
    /// and returns a mutable pointer to the component.
    ///
    /// # Safety
    /// - The caller must ensure that the entity is not already present in the array.
    /// - The caller must uphold safety invariants of [ptr::write](std::ptr::write)
    unsafe fn push(&mut self, entity: Entity) -> PtrMut {
        // TODO: check if I should use `grow` instead
        // grow would use a growth factor to reduce allocation overhead.
        if self.len == self.cap {
            self.reserve(1);
        }

        // SAFETY:
        // * Pointer offset properly calculated.
        // * NonNull ptr safe to write.
        unsafe {
            let row = self.len;
            self.len += 1;
            self.entities.as_ptr().add(row).write(entity);
            self.column.get_mut(row)
        }
    }

    /// Pops the last element from the column.
    /// Returns the entity and component value.
    ///
    /// # Safety
    /// - Caller must ensure that the column is not empty.
    unsafe fn pop_unchecked(&mut self) -> (Entity, OwningPtr) {
        debug_assert!(self.len > 0, "cannot pop from empty column");

        let row = self.len - 1;
        self.len -= 1;

        unsafe {
            let entity = self.entities.as_ptr().add(row).read();
            let ptr = self.column.get_mut(row).promote();

            (entity, ptr)
        }
    }
}

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

/// Sparse set of entities with associated data.
///
/// Provides pointer stability for the data.
pub(crate) struct ComponentSparse {
    dense: Dense,
    pages: Vec<Option<Page>>,
}

impl ComponentSparse {
    pub(crate) fn new(component: Entity, type_info: Rc<TypeInfo>) -> Self {
        // We initialize the sparse set with a single entity
        // The `0` entity is used as the null entity.
        Self {
            dense: Dense::new(component, type_info),
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

    /// Inserts a value for the given entity.
    ///
    /// Caller must ensure that the sparse set stores values of type `C`.
    pub(crate) unsafe fn insert<C: ComponentValue>(
        &mut self,
        entity: Entity,
        value: C,
    ) -> Option<C> {
        let page = Self::ensure_page(&mut self.pages, entity.id());
        let offset = page_offset(entity.id());
        let dense = page[offset];

        if dense != usize::MAX {
            debug_assert!(dense < self.dense.len(), "dense index out of bounds");

            unsafe {
                let dense_entity = self.dense.entities.add(dense).read();

                if dense_entity == entity {
                    let ptr = self.dense.column.get_mut(dense);
                    Some(std::mem::replace(ptr.deref_mut(), value))
                } else {
                    None
                }
            }
        } else {
            page[offset] = self.dense.len();
            unsafe { self.dense.push(entity).write(value) };
            None
        }
    }

    pub(crate) fn remove<C: ComponentValue>(&mut self, entity: Entity) -> Option<C> {
        let page = self
            .pages
            .get_mut(page_index(entity.id()))
            .and_then(Option::as_mut)?;
        let offset = page_offset(entity.id());
        let dense = page[offset];

        // entity not in set.
        if dense == usize::MAX {
            return None;
        }

        debug_assert!(dense < self.dense.len(), "dense index out of bounds");

        // SAFETY: dense is valid.
        let dense_entity = unsafe { self.dense.entities.add(dense).read() };

        // entity generations mismatch.
        if entity != dense_entity {
            return None;
        }

        page[offset] = usize::MAX;

        let last_idx = self.dense.len() - 1;

        if dense != last_idx {
            unsafe {
                swap_entities(&mut self.dense.entities, dense, last_idx);
                self.dense.column.swap_nonoverlapping(dense, last_idx);

                let entity = self.dense.entities.add(dense).read();
                let page = self.pages[page_index(entity.id())]
                    .as_mut()
                    .expect("Sparse set corrupted");

                page[page_offset(entity.id())] = dense;
            }
        }

        // SAFETY: dense array is not empty.
        unsafe {
            let (_, ptr) = self.dense.pop_unchecked();
            Some(ptr.as_ptr().cast::<C>().read())
        }
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

    pub(crate) fn get<C: ComponentValue>(&self, entity: Entity) -> Option<&C> {
        let page = self
            .pages
            .get(page_index(entity.id()))
            .and_then(Option::as_ref)?;
        let dense = page[page_offset(entity.id())];

        if dense == usize::MAX {
            return None;
        }

        debug_assert!(dense < self.dense.len(), "dense index out of bounds");

        unsafe {
            let dense_entity = self.dense.entities.add(dense).read();

            if dense_entity == entity {
                Some(self.dense.column.get(dense).deref())
            } else {
                None
            }
        }
    }

    pub(crate) fn get_mut<C: ComponentValue>(&mut self, entity: Entity) -> Option<&mut C> {
        let page = self
            .pages
            .get(page_index(entity.id()))
            .and_then(Option::as_ref)?;
        let dense = page[page_offset(entity.id())];

        if dense == usize::MAX {
            return None;
        }

        debug_assert!(dense < self.dense.len(), "dense index out of bounds");

        unsafe {
            let dense_entity = self.dense.entities.add(dense).read();

            if dense_entity == entity {
                Some(self.dense.column.get_mut(dense).deref_mut())
            } else {
                None
            }
        }
    }
}
