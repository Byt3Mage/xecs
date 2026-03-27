use crate::{
    component::Component,
    error::EcsResult,
    id::{Entity, Id},
    storage::Storage,
    type_traits::Data,
    world::World,
};

/// Add the id as tag to the entity
///
/// # Safety
/// Caller ensures that id does not have associated data.
pub(crate) fn add_id(world: &mut World, entity: Entity, tag: impl Id) -> EcsResult<()> {
    todo!()
}

/// Sets the value of a component for an id.
///
/// # Safety
/// - Caller must ensure that `val` is the same type and layout of the component.
pub(crate) unsafe fn set_id<T: Component<DataType = Data>>(
    world: &mut World,
    e: Entity,
    id: impl Id,
    val: T,
) -> Option<T> {
    todo!()
}

pub(crate) fn has_id(world: &World, e: Entity, id: impl Id) -> bool {
    let Some(loc) = world.entity_manager.get_location(e) else {
        return false;
    };

    let Some(ci) = id.map_get(&world.components) else {
        return false;
    };

    // SAFETY: Valid id has valid table and row.
    match &ci.storage {
        Storage::SparseTag(set) => set.contains(e),
        Storage::SparseData(set) => set.contains(e),
        Storage::Tables(tables) => tables.contains_key(&loc.table),
    }
}
