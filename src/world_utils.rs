use crate::{
    component::{ComponentValue, TagBuilder},
    entity::Entity,
    error::{EcsError, EcsResult},
    storage::Storage,
    world::World,
};
use const_assert::const_assert;

/// Add the id as tag to the entity
///
/// # Safety
/// Caller ensures that id does not have associated data.
pub(crate) fn add_tag(world: &mut World, entity: Entity, id: Entity) -> EcsResult<()> {
    let r = world.entity_index.get_record(entity)?;

    // Create ComponentRecord for tag if it doesn't exist.
    // Unlike components, tags can be registered on the fly,
    // allowing us to add regular entities without first registering them.
    if !world.components.contains(&id) {
        // TODO: default flags?
        TagBuilder::new(id).build(world);
    }

    // Unwrap should never fail here, as we just ensured the component exists.
    let cr = world.components.get_mut(&id).unwrap();

    debug_assert!(cr.type_info.is_none(), "id has data, can't use add");

    match &mut cr.storage {
        Storage::SparseTag(tag) => Ok(tag.insert(entity)),
        Storage::Tables(_) => todo!(),
        Storage::SparseData(_) => Err(EcsError::ComponentHasData(id)),
    }
}

/// Sets the value of a component for an entity.
///
/// # Safety
/// Caller ensures that the type matches the id.
pub(crate) fn set_component_value<C: ComponentValue>(
    world: &mut World,
    entity: Entity,
    id: Entity,
    value: C,
) -> EcsResult<()> {
    const_assert!(
        |C| size_of::<C>() != 0,
        "can't use set for tag, did you want to add?"
    );

    let cr = match world.components.get_mut(&id) {
        Some(cr) => {
            debug_assert!(
                cr.type_info.as_ref().is_some_and(|ti| ti.is::<C>()),
                "type mismatch"
            );
            cr
        }
        None => return Err(EcsError::UnregisteredComponent(id)),
    };
    let r = world.entity_index.get_record(entity)?;

    // SAFETY:
    // - Caller guarantees that the type matches the id.
    // - Valid entity must have valid table and row.
    match &mut cr.storage {
        Storage::SparseTag(_) => Err(EcsError::ComponentHasNoData(id)),
        Storage::Tables(_) => todo!(),
        Storage::SparseData(set) => Ok(set.insert(entity, value)),
    }
}

/// #Safety
/// Caller ensures that the type matches the id.
pub(crate) fn get_component_value<C: ComponentValue>(
    world: &World,
    entity: Entity,
    id: Entity,
) -> EcsResult<&C> {
    const_assert!(
        |C| size_of::<C>() != 0,
        "can't use get for tag, did you want to check with has?"
    );

    let cr = match world.components.get(&id) {
        Some(cr) => {
            debug_assert!(
                cr.type_info.as_ref().is_some_and(|ti| ti.is::<C>()),
                "type mismatch"
            );
            cr
        }
        None => return Err(EcsError::UnregisteredComponent(id)),
    };
    let r = world.entity_index.get_record(entity)?;

    // SAFETY
    // - Caller guarantees that the type matches the id.
    // - Valid entity must have valid table and row.
    let value = match &cr.storage {
        Storage::SparseTag(_) => return Err(EcsError::ComponentHasNoData(id)),
        Storage::SparseData(set) => set.get(entity),
        Storage::Tables(_) => unsafe { world.table_index[r.table].get(r.row, id) },
    };

    value.ok_or(EcsError::MissingComponent(entity, id))
}

/// #Safety
/// Caller ensures that the type matches the id.
pub(crate) fn get_component_value_mut<C: ComponentValue>(
    world: &mut World,
    entity: Entity,
    id: Entity,
) -> EcsResult<&mut C> {
    const_assert!(
        |C| size_of::<C>() != 0,
        "can't use get_mut for tag, did you want to check with has?"
    );

    let cr = match world.components.get_mut(&id) {
        Some(cr) => {
            debug_assert!(
                cr.type_info.as_ref().is_some_and(|ti| ti.is::<C>()),
                "type mismatch"
            );
            cr
        }
        None => return Err(EcsError::UnregisteredComponent(id)),
    };
    let r = world.entity_index.get_record(entity)?;

    // SAFETY
    // - Caller guarantees that the type matches the id.
    // - Valid entity must have valid row.
    let value = match &mut cr.storage {
        Storage::SparseTag(_) => return Err(EcsError::ComponentHasNoData(id)),
        Storage::SparseData(set) => set.get_mut(entity),
        Storage::Tables(_) => unsafe { world.table_index[r.table].get_mut(r.row, id) },
    };

    value.ok_or(EcsError::MissingComponent(entity, id))
}
