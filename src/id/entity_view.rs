use crate::{
    component::Component,
    error::EcsResult,
    id::Id,
    pointer::{Ptr, PtrMut},
    world::WorldRef,
};

pub struct EntityView<'a> {
    entity: Id,
    world: WorldRef<'a>,
}

impl<'a> EntityView<'a> {
    pub fn new(world: impl Into<WorldRef<'a>>, entity: Id) -> Self {
        Self {
            entity,
            world: world.into(),
        }
    }

    #[inline]
    pub fn id(&self) -> Id {
        self.entity
    }

    #[inline]
    pub fn has(&self, id: impl Into<Id>) -> EcsResult<bool> {
        self.world.has_id(self.entity, id)
    }

    #[inline]
    pub fn has_t<C: Component>(&self) -> EcsResult<bool> {
        self.world.has::<C>(self.entity)
    }

    #[inline]
    pub fn add(mut self, id: impl Into<Id>) -> EcsResult<Self> {
        self.world.add_id(self.entity, id)?;
        Ok(self)
    }

    #[inline]
    pub fn add_t<T: Component>(mut self) -> EcsResult<Self> {
        self.world.add::<T>(self.entity)?;
        Ok(self)
    }

    #[inline]
    pub fn set<C: Component>(&mut self, value: C) -> EcsResult<Option<C>> {
        self.world.set(self.entity, value)
    }

    #[inline]
    pub fn set_id<C>(mut self, id: impl Into<Id>, value: C) -> EcsResult<Self> {
        self.world.set_id(self.entity, id, value)?;
        Ok(self)
    }

    #[inline]
    pub fn get_id(&self, id: impl Into<Id>) -> EcsResult<Ptr> {
        self.world.get_id(self.entity, id)
    }

    #[inline]
    pub fn get<C: Component>(&self) -> EcsResult<&C> {
        self.world.get::<C>(self.entity)
    }

    #[inline]
    pub fn get_id_mut(&mut self, id: impl Into<Id>) -> EcsResult<PtrMut> {
        self.world.get_id_mut(self.entity, id)
    }

    #[inline]
    pub fn get_mut<C: Component>(&mut self) -> EcsResult<&mut C> {
        self.world.get_mut(self.entity)
    }
}
