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
    pub fn has_t<C: ComponentValue>(&self) -> bool {
        self.world.has_t::<C>(self.id)
    }

    #[inline]
    pub fn has(&self, id: Id) -> bool {
        self.world.has(self.id, id)
    }

    #[inline]
    pub fn add_t<C: ComponentValue>(&mut self) -> EcsResult<()> {
        self.world.add_t::<C>(self.id)
    }

    #[inline]
    pub fn add(&mut self, id: Id) -> EcsResult<()> {
        self.world.add(self.id, id)
    }
}