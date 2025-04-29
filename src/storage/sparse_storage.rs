use super::sparse_set::{PagedSparseSet, SparseSet};
use crate::{component::ComponentValue, entity::Entity, type_info::TypeInfo};
use const_assert::const_assert;
use std::{any::TypeId, ptr::NonNull, rc::Rc};

const PAGE_SIZE: usize = 4096;

pub struct ComponentSparseSet {
    inner: NonNull<u8>,
    has_fn: fn(NonNull<u8>, Entity) -> bool,
    drop_fn: fn(NonNull<u8>),
    is_paged: bool,
    type_info: Rc<TypeInfo>,
}

impl Drop for ComponentSparseSet {
    fn drop(&mut self) {
        (self.drop_fn)(self.inner)
    }
}

impl ComponentSparseSet {
    fn new<C: ComponentValue>(type_info: Rc<TypeInfo>, set: SparseSet<C>) -> Self {
        const_assert!(
            |C| size_of::<C>() != 0,
            "Component type cannot be zero-sized, use TagSparseSet"
        );

        assert!(
            type_info.type_id == TypeId::of::<C>(),
            "component type info does not match sparse set type"
        );

        let inner = {
            let boxed = Box::new(set);
            // Safety: Box::into_raw returns a non-null pointer
            unsafe { NonNull::new_unchecked(Box::into_raw(boxed) as *mut u8) }
        };

        let has_fn = |ptr: NonNull<u8>, entity: Entity| unsafe {
            ptr.cast::<SparseSet<C>>().as_ref().has_entity(entity)
        };

        let drop_fn = |ptr: NonNull<u8>| {
            // Safety: We know that the pointer is valid and points to a SparseSet<C>
            unsafe {
                ptr.cast::<SparseSet<C>>().drop_in_place();
            }
        };

        Self {
            inner,
            has_fn,
            drop_fn,
            is_paged: false,
            type_info,
        }
    }

    fn new_paged<C: ComponentValue>(
        type_info: Rc<TypeInfo>,
        set: PagedSparseSet<C, PAGE_SIZE>,
    ) -> Self {
        const_assert!(
            |C| size_of::<C>() != 0,
            "Component type cannot be zero-sized, use TagSparseSet"
        );

        assert!(
            type_info.type_id == TypeId::of::<C>(),
            "component type info does not match sparse set type"
        );

        let inner = {
            let boxed = Box::new(set);
            // Safety: Box::into_raw returns a non-null pointer
            unsafe { NonNull::new_unchecked(Box::into_raw(boxed) as *mut u8) }
        };

        let has_fn = |ptr: NonNull<u8>, entity: Entity| {
            // Safety: We know that the pointer is valid and points to a SparseSet<C>
            unsafe {
                ptr.cast::<PagedSparseSet<C, PAGE_SIZE>>()
                    .as_ref()
                    .has_entity(entity)
            }
        };

        let drop_fn = |set: NonNull<u8>| {
            // Safety: We know that the pointer is valid and points to a SparseSet<C>
            unsafe {
                set.cast::<PagedSparseSet<C, PAGE_SIZE>>().drop_in_place();
            }
        };

        Self {
            inner,
            has_fn,
            drop_fn,
            is_paged: true,
            type_info,
        }
    }

    /// Inserts a value into the sparse storage.
    ///
    /// # Safety
    /// The caller must ensure that the entity is valid and that the value is of the correct type.
    #[inline]
    pub(crate) unsafe fn insert<C: ComponentValue>(
        &mut self,
        entity: Entity,
        value: C,
    ) -> Option<C> {
        #[cfg(debug_assertions)]
        debug_assert!(self.type_info.type_id == TypeId::of::<C>(), "Type mismatch");

        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<C, PAGE_SIZE>>().as_mut();
                set.insert(entity, value)
            } else {
                let set = self.inner.cast::<SparseSet<C>>().as_mut();
                set.insert(entity, value)
            }
        }
    }

    /// Removes a value from the sparse storage.
    ///
    /// # Safety
    /// The caller must ensure that the value is of the correct type.
    #[inline]
    pub(crate) unsafe fn remove<C: ComponentValue>(&mut self, entity: Entity) -> Option<C> {
        #[cfg(debug_assertions)]
        debug_assert!(self.type_info.type_id == TypeId::of::<C>(), "Type mismatch");

        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<C, PAGE_SIZE>>().as_mut();
                set.remove(entity)
            } else {
                let set = self.inner.cast::<SparseSet<C>>().as_mut();
                set.remove(entity)
            }
        }
    }

    /// Checks if a value exists in the sparse storage.
    #[inline]
    pub(crate) fn has(&self, entity: Entity) -> bool {
        (self.has_fn)(self.inner, entity)
    }

    /// Retrieves a value from the sparse storage.
    ///
    /// # Safety
    /// The caller must ensure that the value is of the correct type.
    #[inline]
    pub(crate) unsafe fn get<C: ComponentValue>(&self, entity: Entity) -> Option<&C> {
        #[cfg(debug_assertions)]
        debug_assert!(self.type_info.type_id == TypeId::of::<C>(), "Type mismatch");

        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<C, PAGE_SIZE>>().as_ref();
                set.get(entity)
            } else {
                let set = self.inner.cast::<SparseSet<C>>().as_ref();
                set.get(entity)
            }
        }
    }

    /// Retrieves a mutable value from the sparse storage.
    ///
    /// # Safety
    /// The caller must ensure that the value is of the correct type.
    #[inline]
    pub(crate) unsafe fn get_mut<C: ComponentValue>(&mut self, entity: Entity) -> Option<&mut C> {
        #[cfg(debug_assertions)]
        debug_assert!(self.type_info.type_id == TypeId::of::<C>(), "Type mismatch");

        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<C, PAGE_SIZE>>().as_mut();
                set.get_mut(entity)
            } else {
                let set = self.inner.cast::<SparseSet<C>>().as_mut();
                set.get_mut(entity)
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

impl From<SparseSet<()>> for TagSparseSet {
    fn from(set: SparseSet<()>) -> Self {
        let inner = {
            let boxed = Box::new(set);
            // Safety: Box::into_raw returns a non-null pointer
            unsafe { NonNull::new_unchecked(Box::into_raw(boxed) as *mut u8) }
        };

        let drop_fn = |ptr: NonNull<u8>| {
            // Safety: We know that the pointer is valid and points to a SparseSet<()>
            unsafe {
                ptr.cast::<SparseSet<()>>().drop_in_place();
            }
        };

        Self {
            inner,
            drop_fn,
            is_paged: false,
        }
    }
}

impl From<PagedSparseSet<(), PAGE_SIZE>> for TagSparseSet {
    fn from(set: PagedSparseSet<(), PAGE_SIZE>) -> Self {
        let inner = {
            let boxed = Box::new(set);
            // Safety: Box::into_raw returns a non-null pointer
            unsafe { NonNull::new_unchecked(Box::into_raw(boxed) as *mut u8) }
        };

        let drop_fn = |ptr: NonNull<u8>| {
            // Safety: We know that the pointer is valid and points to a SparseSet<()>
            unsafe {
                ptr.cast::<PagedSparseSet<(), PAGE_SIZE>>().drop_in_place();
            }
        };

        Self {
            inner,
            drop_fn,
            is_paged: true,
        }
    }
}

impl TagSparseSet {
    /// Inserts a value into the sparse storage.
    #[inline]
    pub(crate) fn insert(&mut self, entity: Entity) -> Option<()> {
        // SAFETY: We only construct the set using the correct type.
        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<(), PAGE_SIZE>>().as_mut();
                set.insert(entity, ())
            } else {
                let set = self.inner.cast::<SparseSet<()>>().as_mut();
                set.insert(entity, ())
            }
        }
    }

    /// Removes a value from the sparse storage.
    #[inline]
    pub(crate) fn remove(&mut self, entity: Entity) -> Option<()> {
        // SAFETY: We only construct the set using the correct type.
        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<(), PAGE_SIZE>>().as_mut();
                set.remove(entity)
            } else {
                let set = self.inner.cast::<SparseSet<()>>().as_mut();
                set.remove(entity)
            }
        }
    }

    /// Checks if a value exists in the sparse storage.
    #[inline]
    pub(crate) fn has(&self, entity: Entity) -> bool {
        // SAFETY: We only construct the set using the correct type.
        unsafe {
            if self.is_paged {
                let set = self.inner.cast::<PagedSparseSet<(), PAGE_SIZE>>().as_ref();
                set.has_entity(entity)
            } else {
                let set = self.inner.cast::<SparseSet<()>>().as_ref();
                set.has_entity(entity)
            }
        }
    }
}
