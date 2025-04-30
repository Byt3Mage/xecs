use crate::{
    component::{Component, ComponentValue, Tag},
    entity::Entity,
    error::EcsResult,
    world::WorldRef,
};

pub struct EntityView<'a> {
    entity: Entity,
    world: WorldRef<'a>,
}

impl<'a> EntityView<'a> {
    pub fn new(world: impl Into<WorldRef<'a>>, entity: Entity) -> Self {
        Self {
            entity,
            world: world.into(),
        }
    }

    #[inline]
    pub fn id(&self) -> Entity {
        self.entity
    }

    #[inline]
    pub fn has(&self, id: Entity) -> EcsResult<bool> {
        self.world.has(self.entity, id)
    }

    #[inline]
    pub fn has_t<C: ComponentValue>(&self) -> EcsResult<bool> {
        self.world.has_t::<C>(self.entity)
    }

    #[inline]
    pub fn add(mut self, tag: Tag) -> EcsResult<Self> {
        self.world.add(self.entity, tag)?;
        Ok(self)
    }

    #[inline]
    pub fn add_t<T: ComponentValue>(mut self) -> EcsResult<Self> {
        self.world.add_t::<T>(self.entity)?;
        Ok(self)
    }

    #[inline]
    pub fn set_t<C: ComponentValue>(mut self, value: C) -> EcsResult<Self> {
        self.world.set_t(self.entity, value)?;
        Ok(self)
    }

    #[inline]
    pub fn set<C: ComponentValue>(mut self, component: Component<C>, value: C) -> EcsResult<Self> {
        self.world.set(self.entity, component, value)?;
        Ok(self)
    }

    #[inline]
    pub fn get<C: ComponentValue>(&mut self, component: Component<C>) -> EcsResult<&C> {
        self.world.get(self.entity, component)
    }

    #[inline]
    pub fn get_t<C: ComponentValue>(&mut self) -> EcsResult<&C> {
        self.world.get_t(self.entity)
    }

    #[inline]
    pub fn get_mut<C: ComponentValue>(&mut self, component: Component<C>) -> EcsResult<&mut C> {
        self.world.get_mut(self.entity, component)
    }

    #[inline]
    pub fn get_mut_t<C: ComponentValue>(&mut self) -> EcsResult<&mut C> {
        self.world.get_mut_t(self.entity)
    }
}
