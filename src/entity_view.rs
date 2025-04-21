use crate::{component::ComponentValue, entity::Entity, error::EcsResult, id::Id, world::WorldRef};

pub struct EntityView<'a> {
    id: Entity,
    world: WorldRef<'a>
}

impl <'a> EntityView<'a> {
    pub fn new(world: impl Into<WorldRef<'a>>, id: Entity) -> Self {
        Self {
            id,
            world: world.into()
        }
    }

    #[inline]
    pub fn id(&self) -> Id {
        self.id
    }

    #[inline]
    pub fn has(&self, id: Id) -> bool {
        self.world.has(self.id, id)
    }

    #[inline]
    pub fn has_t<C: ComponentValue>(&self) -> bool {
        self.world.has_t::<C>(self.id)
    }

    #[inline]
    pub fn has_p(&self, rel: Id, obj: Id) -> bool {
        self.world.has_p(self.id, rel, obj)
    }

    #[inline]
    pub fn add(mut self, id: Id) -> EcsResult<Self> {
        self.world.add(self.id, id)?;
        Ok(self)
    }

    #[inline]
    pub fn add_t<C: ComponentValue>(mut self) -> EcsResult<Self> {
        self.world.add_t::<C>(self.id)?;
        Ok(self)
    }

    #[inline]
    pub fn add_r(mut self, rel: Id, obj: Id) -> EcsResult<Self> {
        self.world.add_r(self.id, rel, obj)?;
        Ok(self)
    }

    #[inline]
    pub fn set_t<C: ComponentValue>(mut self, value: C) -> EcsResult<Self> {
        self.world.set_t(self.id, value)?;
        Ok(self)
    }

    #[inline]
    pub fn set<C: ComponentValue>(mut self, id: Id, value: C) -> EcsResult<Self> {
        self.world.set(self.id, id, value)?;
        Ok(self)
    }

    #[inline]
    pub fn get<C: ComponentValue>(&mut self, id: Id) -> EcsResult<&C> {
        self.world.get(self.id, id)
    }

    #[inline]
    pub fn get_t<C: ComponentValue>(&mut self) -> EcsResult<&C> {
        self.world.get_t(self.id)
    }

    #[inline]
    pub fn get_mut<C: ComponentValue>(&mut self, id: Id) -> EcsResult<&mut C> {
        self.world.get_mut(self.id, id)
    }

    #[inline]
    pub fn get_mut_t<C: ComponentValue>(&mut self) -> EcsResult<&mut C> {
        self.world.get_mut_t(self.id)
    }
}