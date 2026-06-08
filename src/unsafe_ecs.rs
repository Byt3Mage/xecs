use std::marker::PhantomData;

use crate::ecs::Ecs;

#[derive(Copy, Clone)]
pub struct UnsafeEcsCell<'e>(*mut Ecs, PhantomData<&'e Ecs>);

impl<'w> UnsafeEcsCell<'w> {
    pub fn new(ecs: &'w Ecs) -> Self {
        Self(ecs as *const Ecs as *mut Ecs, PhantomData)
    }

    pub fn new_mut(ecs: &'w mut Ecs) -> Self {
        Self(ecs as *mut Ecs, PhantomData)
    }

    /// # Safety
    /// Caller must ensure no mutable access to the same data exists.
    pub unsafe fn ecs(&self) -> &'w Ecs {
        unsafe { &*self.0 }
    }

    /// # Safety
    /// Caller must ensure exclusive access to the requested data.
    pub unsafe fn world_mut(&self) -> &'w mut Ecs {
        unsafe { &mut *self.0 }
    }
}
