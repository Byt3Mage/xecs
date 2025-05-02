use crate::{entity::Entity, error::EcsResult, type_info::TypeInfo, world::World};
use std::sync::atomic::AtomicUsize;

static MAX_INDEX: AtomicUsize = AtomicUsize::new(0);

pub trait TypeImpl: 'static {
    fn index() -> usize;
    fn is_registered(world: &World) -> bool;
    fn register(world: &mut World, entity: Entity);
    fn id(world: &World) -> EcsResult<Entity>;
    fn type_info() -> &'static TypeInfo;
}

macro_rules! impl_type {
    ($ty: path) => {
        impl TypeImpl for $ty {
            fn index() -> usize {
                static INDEX: LazyLock<usize> =
                    LazyLock::new(|| MAX_INDEX.fetch_add(1, Ordering::Relaxed));
                *INDEX
            }

            fn is_registered(world: &World) -> bool {
                world
                    .type_ids
                    .get(Self::index())
                    .map_or(false, |&id| id != Entity::NULL)
            }

            fn register(world: &mut World, entity: Entity) {
                assert!(
                    !Self::is_registered(world),
                    "component '{}' is already registered",
                    std::any::type_name::<Self>()
                );

                let index = Self::index();

                if index >= world.type_ids.len() {
                    world.type_ids.resize(index + 1, Entity::NULL);
                }

                world.type_ids[index] = entity;
            }

            fn id(world: &World) -> EcsResult<Entity> {
                match world.type_ids.get(<Self as TypeImpl>::index()) {
                    Some(&id) if id != Entity::NULL => {
                        debug_assert!(
                            world.entity_index.is_alive(id),
                            "component '{}' was deleted, re-register before using",
                            std::any::type_name::<Self>()
                        );

                        Ok(id)
                    }
                    _ => Err(EcsError::UnregisteredType(std::any::type_name::<Self>())),
                }
            }

            fn type_info() -> &'static TypeInfo {
                static TYPE_INFO: OnceLock<TypeInfo> = OnceLock::new();
                TYPE_INFO.get_or_init(|| TypeInfo::new::<Self>())
            }
        }
    };
}
