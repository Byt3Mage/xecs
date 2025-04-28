use std::{
    marker::PhantomData,
    sync::{
        LazyLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{entity::Entity, world::World};

static MAX_INDEX: AtomicUsize = AtomicUsize::new(0);

pub struct TypeImpl<T> {
    _marker: PhantomData<T>,
}

impl<T> TypeImpl<T> {
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
    ///
    /// # Panics
    /// Panics if the component is not registered.
    pub fn id(world: &World) -> Entity {
        let id = match world.type_ids.get(Self::index()) {
            Some(&id) if id != Entity::NULL => id,
            _ => panic!(
                "component '{}' must be registered before use",
                std::any::type_name::<T>()
            ),
        };

        // TODO: consider making it a full assert
        debug_assert!(
            world.entity_index.is_alive(id),
            "component '{}' was deleted, re-register before using",
            std::any::type_name::<T>()
        );

        id
    }

    pub fn try_id(world: &World) -> Option<Entity> {
        world
            .type_ids
            .get(Self::index())
            .filter(|&&id| world.entity_index.is_alive(id))
            .copied()
    }
}
