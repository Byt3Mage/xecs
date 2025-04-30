use std::{
    marker::PhantomData,
    sync::{
        LazyLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{
    entity::Entity,
    error::{EcsError, EcsResult},
    world::World,
};

static MAX_INDEX: AtomicUsize = AtomicUsize::new(0);

pub struct TypeImpl<T: 'static> {
    _marker: PhantomData<T>,
}

impl<T: 'static> TypeImpl<T> {
    pub(crate) fn index() -> usize {
        static VALUE: LazyLock<usize> = LazyLock::new(|| MAX_INDEX.fetch_add(1, Ordering::Relaxed));
        *VALUE
    }

    #[inline]
    pub(crate) fn is_registered(world: &World) -> bool {
        world
            .type_ids
            .get(Self::index())
            .map_or(false, |&id| id != Entity::NULL)
    }

    pub(crate) fn register(world: &mut World, entity: Entity) {
        assert!(
            !Self::is_registered(world),
            "component '{}' is already registered",
            std::any::type_name::<T>()
        );

        let index = Self::index();

        if index >= world.type_ids.len() {
            world.type_ids.resize(index + 1, Entity::NULL);
        }

        world.type_ids[index] = entity;
    }

    /// Gets the entity ID of the component type.
    pub fn id(world: &World) -> EcsResult<Entity> {
        let id = world
            .type_ids
            .get(Self::index())
            .copied()
            .ok_or_else(|| EcsError::UnregisteredType(std::any::type_name::<T>()))?;

        assert!(
            world.entity_index.is_alive(id),
            "component '{}' was deleted, re-register before using",
            std::any::type_name::<T>()
        );

        Ok(id)
    }
}
