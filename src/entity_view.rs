use crate::{component::ComponentValue, entity::Entity, world::WorldRef};

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

    pub fn get<T: ComponentValue>(&self) -> Option<&T> {
        todo!()
    }

    pub fn get_mut<T: ComponentValue>(&mut self) -> Option<&mut T> {
        todo!()
    }
}