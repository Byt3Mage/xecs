use crate::{
    error::{UnregisteredTypeErr, unreg_type_err},
    id::Id,
    type_traits::Component,
    world::World,
};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeIndex(usize);

impl TypeIndex {
    pub const INVALID: Self = TypeIndex(usize::MAX);
}

pub fn allocate_type_index() -> TypeIndex {
    static MAX_INDEX: AtomicUsize = AtomicUsize::new(0);
    TypeIndex(MAX_INDEX.fetch_add(1, Ordering::Relaxed))
}

/// # Safety
/// DO NOT implement this trait directly, use #\[derive(Component)\] instead.
pub unsafe trait ComponentId: Component {
    #[doc(hidden)]
    fn type_index() -> TypeIndex;

    #[doc(hidden)]
    fn id(world: &World) -> Result<Id, UnregisteredTypeErr> {
        if !Self::IS_GENERIC {
            match world.type_arr.get(Self::type_index().0) {
                Some(&Some(id)) => Ok(id),
                _ => Err(unreg_type_err::<Self>()),
            }
        } else {
            match world.type_map.get::<Self>() {
                Some(&id) => Ok(id),
                None => Err(unreg_type_err::<Self>()),
            }
        }
    }

    fn get_or_register_type(world: &mut World) -> Id {
        if !Self::IS_GENERIC {
            let index = Self::type_index().0;

            if index >= world.type_arr.len() {
                world.type_arr.resize(index + 1, None);
            }

            match world.type_arr[index] {
                Some(id) => id,
                None => {
                    let new_id = world.new_id();
                    world.type_arr[index] = Some(new_id);
                    new_id
                }
            }
        } else {
            match world.type_map.get::<Self>() {
                Some(&id) => id,
                None => {
                    let new_id = world.new_id();
                    world.type_map.insert::<Self>(new_id);
                    new_id
                }
            }
        }
    }
}
