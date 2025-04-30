use super::sparse_set::{Entry, PagedSparseSet, SparseSet};
use crate::{
    component::ComponentValue,
    entity::Entity,
    error::{EcsError, EcsResult},
};
use const_assert::const_assert;
use std::ptr::NonNull;

const PAGE_SIZE: usize = 4096;

pub struct ComponentSparseSet {
    inner: NonNull<u8>,
    has_fn: fn(NonNull<u8>, &Entity) -> bool,
    drop_fn: fn(NonNull<u8>),
    is_paged: bool,
}

impl Drop for ComponentSparseSet {
    fn drop(&mut self) {
        (self.drop_fn)(self.inner)
    }
}

impl ComponentSparseSet {
    pub(crate) fn new<C: ComponentValue>() -> Self {
        const_assert!(
            |C| size_of::<C>() != 0,
            "Component type cannot be zero-sized, use TagSparseSet"
        );

        let inner = {
            let boxed = Box::new(SparseSet::<Entity, C>::new());
            // Safety: Box::into_raw returns a non-null pointer
            unsafe { NonNull::new_unchecked(Box::into_raw(boxed) as *mut u8) }
        };

        // SAFETY: we are constructing this struct with the right pointer type
        let has_fn = |ptr: NonNull<u8>, entity: &Entity| {
            // Safety: We know that the pointer is valid and points to a PagedSparseSet<Entity, C>
            unsafe {
                ptr.cast::<PagedSparseSet<Entity, C>>()
                    .as_ref()
                    .contains(entity)
            }
        };

        let drop_fn = |ptr: NonNull<u8>| {
            // Safety: We know that the pointer is valid and points to a SparseSet<Entity, C>
            unsafe {
                ptr.cast::<SparseSet<Entity, C>>().drop_in_place();
            }
        };

        Self {
            inner,
            has_fn,
            drop_fn,
            is_paged: false,
        }
    }

    pub(crate) fn new_paged<C: ComponentValue>(page_size: usize) -> Self {
        const_assert!(
            |C| size_of::<C>() != 0,
            "Component type cannot be zero-sized, use TagSparseSet"
        );

        let inner = {
            let boxed = Box::new(PagedSparseSet::<Entity, C>::new(page_size));
            // Safety: Box::into_raw returns a non-null pointer
            unsafe { NonNull::new_unchecked(Box::into_raw(boxed) as *mut u8) }
        };

        let has_fn = |ptr: NonNull<u8>, entity: &Entity| {
            // Safety: We know that the pointer is valid and points to a PagedSparseSet<Entity, C>
            unsafe {
                ptr.cast::<PagedSparseSet<Entity, C>>()
                    .as_ref()
                    .contains(entity)
            }
        };

        let drop_fn = |set: NonNull<u8>| {
            // Safety: We know that the pointer is valid and points to a PagedSparseSet<Entity, C>
            unsafe {
                set.cast::<PagedSparseSet<Entity, C>>().drop_in_place();
            }
        };

        Self {
            inner,
            has_fn,
            drop_fn,
            is_paged: true,
        }
    }

    /// Inserts a value into the sparse storage.
    /// Returns a tuple containing a boolean indicating whether the insertion was successful,
    /// and an optional value that was previously associated with the entity.
    ///
    /// # Safety
    /// The caller must ensure that the entity is valid and that the value is of the correct type.
    #[inline]
    pub(crate) unsafe fn insert<C: ComponentValue>(&mut self, entity: Entity, value: C) {
        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<Entity, C>>().as_mut();
                set.insert(entity, value);
            } else {
                let set = self.inner.cast::<SparseSet<Entity, C>>().as_mut();
                set.insert(entity, value);
            }
        }
    }

    /// Removes a value from the sparse storage.
    ///
    /// # Safety
    /// The caller must ensure that the value is of the correct type.
    #[inline]
    pub(crate) unsafe fn remove<C: ComponentValue>(
        &mut self,
        entity: &Entity,
    ) -> Option<Entry<Entity, C>> {
        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<Entity, C>>().as_mut();
                set.remove(entity)
            } else {
                let set = self.inner.cast::<SparseSet<Entity, C>>().as_mut();
                set.remove(entity)
            }
        }
    }

    /// Checks if a value exists in the sparse storage.
    #[inline]
    pub(crate) fn has(&self, entity: &Entity) -> bool {
        (self.has_fn)(self.inner, entity)
    }

    /// Retrieves a value from the sparse storage.
    ///
    /// # Safety
    /// The caller must ensure that the value is of the correct type.
    #[inline]
    pub(crate) unsafe fn get<C: ComponentValue>(
        &self,
        entity: Entity,
        id: Entity,
    ) -> EcsResult<&C> {
        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<Entity, C>>().as_ref();
                set.get(&entity)
                    .ok_or(EcsError::MissingComponent(entity, id))
            } else {
                let set = self.inner.cast::<SparseSet<Entity, C>>().as_ref();
                set.get(&entity)
                    .ok_or(EcsError::MissingComponent(entity, id))
            }
        }
    }

    /// Retrieves a mutable value from the sparse storage.
    ///
    /// # Safety
    /// The caller must ensure that the value is of the correct type.
    #[inline]
    pub(crate) unsafe fn get_mut<C: ComponentValue>(
        &mut self,
        entity: Entity,
        id: Entity,
    ) -> EcsResult<&mut C> {
        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<Entity, C>>().as_mut();
                set.get_mut(&entity)
                    .ok_or(EcsError::MissingComponent(entity, id))
            } else {
                let set = self.inner.cast::<SparseSet<Entity, C>>().as_mut();
                set.get_mut(&entity)
                    .ok_or(EcsError::MissingComponent(entity, id))
            }
        }
    }
}

pub struct TagSparseSet {
    inner: NonNull<u8>,
    drop_fn: fn(NonNull<u8>),
    is_paged: bool,
}

impl Drop for TagSparseSet {
    fn drop(&mut self) {
        (self.drop_fn)(self.inner)
    }
}

impl TagSparseSet {
    pub(crate) fn new() -> Self {
        let inner = {
            let boxed = Box::new(SparseSet::<Entity, ()>::new());
            // Safety: Box::into_raw returns a non-null pointer
            unsafe { NonNull::new_unchecked(Box::into_raw(boxed) as *mut u8) }
        };

        let drop_fn = |ptr: NonNull<u8>| {
            // Safety: We know that the pointer is valid and points to a SparseSet<()>
            unsafe {
                ptr.cast::<SparseSet<Entity, ()>>().drop_in_place();
            }
        };

        Self {
            inner,
            drop_fn,
            is_paged: false,
        }
    }

    pub(crate) fn new_paged(page_size: usize) -> Self {
        let inner = {
            let boxed = Box::new(PagedSparseSet::<Entity, ()>::new(page_size));
            // Safety: Box::into_raw returns a non-null pointer
            unsafe { NonNull::new_unchecked(Box::into_raw(boxed) as *mut u8) }
        };

        let drop_fn = |ptr: NonNull<u8>| {
            // Safety: We know that the pointer is valid and points to a SparseSet<()>
            unsafe {
                ptr.cast::<PagedSparseSet<Entity, ()>>().drop_in_place();
            }
        };

        Self {
            inner,
            drop_fn,
            is_paged: true,
        }
    }

    /// Inserts a value into the sparse storage.
    ///
    /// Returns `true` if the value was newly inserted, `false` if it already existed.
    #[inline]
    pub(crate) fn insert(&mut self, entity: Entity) {
        // SAFETY: We only construct the set using the correct type.
        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<Entity, ()>>().as_mut();
                set.insert(entity, ());
            } else {
                let set = self.inner.cast::<SparseSet<Entity, ()>>().as_mut();
                set.insert(entity, ());
            }
        }
    }

    /// Removes a value from the sparse storage.
    #[inline]
    pub(crate) fn remove(&mut self, entity: &Entity) -> bool {
        // SAFETY: We only construct the set using the correct type.
        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<Entity, ()>>().as_mut();
                set.remove(entity).is_some()
            } else {
                let set = self.inner.cast::<SparseSet<Entity, ()>>().as_mut();
                set.remove(entity).is_some()
            }
        }
    }

    /// Checks if a value exists in the sparse storage.
    #[inline]
    pub(crate) fn has(&self, entity: &Entity) -> bool {
        // SAFETY: We only construct the set using the correct type.
        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<Entity, ()>>().as_ref();
                set.contains(entity)
            } else {
                let set = self.inner.cast::<SparseSet<Entity, ()>>().as_ref();
                set.contains(entity)
            }
        }
    }
}
