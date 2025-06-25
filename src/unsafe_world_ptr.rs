use crate::{
    error::InvalidId,
    id::{Id, id_index::IdLocation},
    world::World,
};
use std::{cell::UnsafeCell, marker::PhantomData};

#[derive(Clone, Copy)]
pub struct UnsafeWorldPtr<'w> {
    ptr: *mut World,
    lifetime: PhantomData<(&'w World, &'w UnsafeCell<World>)>,
}

impl<'w> From<&'w World> for UnsafeWorldPtr<'w> {
    fn from(value: &'w World) -> Self {
        Self {
            ptr: std::ptr::from_ref(value).cast_mut(),
            lifetime: PhantomData,
        }
    }
}

impl<'w> From<&'w mut World> for UnsafeWorldPtr<'w> {
    fn from(value: &'w mut World) -> Self {
        Self {
            ptr: std::ptr::from_mut(value),
            lifetime: PhantomData,
        }
    }
}

impl<'w> UnsafeWorldPtr<'w> {
    /// TODO: documentation
    #[inline]
    pub(crate) unsafe fn world_mut(self) -> &'w mut World {
        // TODO: self.assert_allows_mutable_access();
        // SAFETY:
        // - caller ensures the created `&mut World` is the only borrow of world
        unsafe { &mut *self.ptr }
    }

    /// Gets a reference to the [`&World`](World) this [`UnsafeWorldPtr`] belongs to.
    /// This can be used for arbitrary shared/readonly access.
    ///
    /// # Safety
    /// - must have permission to access the whole world immutably
    /// - there must be no live exclusive borrows on any world data
    /// - there must be no live exclusive borrow of world
    pub(crate) unsafe fn world(self) -> &'w World {
        unsafe { &*self.ptr }
    }

    #[inline]
    unsafe fn get_world(self) -> &'w World {
        // SAFETY:
        // - caller ensures that the returned `&World` is not does not conflict
        //   with any existing mutable borrows of world data
        unsafe { &*self.ptr }
    }

    #[inline]
    pub(crate) fn get_id_location(self, id: Id) -> Result<IdLocation, InvalidId> {
        unsafe { self.get_world().id_index.get_location(id) }
    }
}
