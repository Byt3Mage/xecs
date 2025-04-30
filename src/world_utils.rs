use std::any::TypeId;

use crate::{
    component::ComponentValue,
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
    let cr = match world.components.get_mut(&id) {
        Some(cr) => cr,
        None => return Err(EcsError::UnregisteredComponent(id)),
    };

    debug_assert!(
        cr.type_info.is_none(),
        "id has associated data, can't use add"
    );

    let r = world.entity_index.get_record(entity)?;

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
        Some(cr) => cr,
        None => return Err(EcsError::UnregisteredComponent(id)),
    };
    let r = world.entity_index.get_record(entity)?;

    debug_assert!(
        cr.type_info
            .as_ref()
            .is_some_and(|ti| ti.type_id == TypeId::of::<C>()),
        "type mismatch"
    );

    // SAFETY:
    // - Caller guarantees that the type matches the id.
    // - Valid entity must have valid table and row.
    match &mut cr.storage {
        Storage::SparseTag(_) => Err(EcsError::ComponentHasNoData(id)),
        Storage::Tables(_) => todo!(),
        Storage::SparseData(data) => unsafe { Ok(data.insert(entity, value)) },
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
        Some(cr) => cr,
        None => return Err(EcsError::UnregisteredComponent(id)),
    };
    let r = world.entity_index.get_record(entity)?;

    debug_assert!(
        cr.type_info
            .as_ref()
            .is_some_and(|ti| ti.type_id == TypeId::of::<C>()),
        "type mismatch"
    );

    // SAFETY
    // - Caller guarantees that the type matches the id.
    // - Valid entity must have valid table and row.
    match &cr.storage {
        Storage::SparseTag(_) => Err(EcsError::ComponentHasNoData(id)),
        Storage::SparseData(set) => unsafe { set.get::<C>(entity, id) },
        Storage::Tables(_) => unsafe { world.table_index[r.table].get::<C>(r.row, id) },
    }
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
        Some(cr) => cr,
        None => return Err(EcsError::UnregisteredComponent(id)),
    };
    let r = world.entity_index.get_record(entity)?;

    debug_assert!(
        cr.type_info
            .as_ref()
            .is_some_and(|ti| ti.type_id == TypeId::of::<C>()),
        "type mismatch"
    );

    // SAFETY
    // - Caller guarantees that the type matches the id.
    // - Valid entity must have valid row.
    match &mut cr.storage {
        Storage::SparseTag(_) => Err(EcsError::ComponentHasNoData(id)),
        Storage::SparseData(set) => unsafe { set.get_mut::<C>(entity, id) },
        Storage::Tables(_) => unsafe { world.table_index[r.table].get_mut::<C>(r.row, id) },
    }
}
