use crate::{
    component::ensure_component,
    error::{EcsError, EcsResult},
    graph::table_traverse_add,
    id::Id,
    pointer::{Ptr, PtrMut},
    storage::{Storage, table::move_entity},
    world::World,
};
use const_assert::const_assert;

/// Add the id as tag to the entity
///
/// # Safety
/// Caller ensures that id does not have associated data.
pub(crate) fn add_tag(world: &mut World, entity: Id, id: Id) -> EcsResult<()> {
    let r = world.id_index.get_record(entity)?;

    // We cache the table and row here so if get record fails,
    // we don't create a component for id
    let src_table = r.table;
    let src_row = r.row;

    // Create ComponentRecord for tag if it doesn't exist.
    // Unlike components, tags can be registered on the fly,
    // allowing us to add regular entities as tags without first registering them.
    ensure_component(world, id);

    let ci = world.components.get_mut(id).unwrap();

    // SAFETY: we just checked that the id is a tag.
    match &mut ci.storage {
        Storage::SparseTag(set) => Ok(set.insert(id)),
        Storage::SparseData(_) => Err(EcsError::IsNotTag(id)),
        Storage::Tables(_) => {
            if let Some(dst_table) = table_traverse_add(world, src_table, id) {
                // SAFETY
                // - We ensured that dst_table is not the same as src.
                // - entity is valid, which means that src_row must be valid.
                unsafe { move_entity(world, entity, src_table, src_row, dst_table) };
            }

            // Does nothing if there's no destination table.
            // This means that the entity already contains the tag.
            Ok(())
        }
    }
}

/// Sets the value of a component for an entity.
pub(crate) fn set_component<C>(
    world: &mut World,
    entity: Id,
    id: Id,
    val: C,
) -> EcsResult<Option<C>> {
    const_assert!(
        |C| size_of::<C>() != 0,
        "can't use set for tag, did you want to add?"
    );

    let r = world.id_index.get_record(entity)?;
    let ci = match world.components.get_mut(id) {
        Some(ci) => ci,
        None => return Err(EcsError::IdNotComponent(id)),
    };

    // SAFETY: Valid entity must have valid table and row.
    match &mut ci.storage {
        Storage::SparseTag(_) => Err(EcsError::IsTag(id)),
        Storage::SparseData(set) => Ok(unsafe { set.insert(entity, val) }),
        Storage::Tables(_) => todo!(),
    }
}

pub(crate) fn get_component(world: &World, entity: Id, id: Id) -> EcsResult<Ptr> {
    let r = world.id_index.get_record(entity)?;
    let ci = match world.components.get(id) {
        Some(ci) => ci,
        None => return Err(EcsError::IdNotComponent(id)),
    };

    // SAFETY: Valid entity must have valid table and row.
    match &ci.storage {
        Storage::SparseTag(_) => Err(EcsError::IsTag(id)),
        Storage::SparseData(set) => set.get_ptr(entity),
        Storage::Tables(_) => unsafe { world.table_index[r.table].get_ptr(r.row, id) },
    }
}

pub(crate) fn get_component_mut(world: &mut World, entity: Id, id: Id) -> EcsResult<PtrMut> {
    let r = world.id_index.get_record(entity)?;
    let ci = match world.components.get_mut(id) {
        Some(ci) => ci,
        None => return Err(EcsError::IdNotComponent(id)),
    };

    // SAFETY: Valid entity must have valid table and row.
    match &mut ci.storage {
        Storage::SparseTag(_) => return Err(EcsError::IsTag(id)),
        Storage::SparseData(set) => set.get_ptr_mut(entity),
        Storage::Tables(_) => unsafe { world.table_index[r.table].get_ptr_mut(r.row, id) },
    }
}

pub(crate) fn has_component(world: &World, entity: Id, id: Id) -> EcsResult<bool> {
    let r = world.id_index.get_record(entity)?;
    let cr = match world.components.get(id) {
        Some(cr) => cr,
        None => return Err(EcsError::IdNotComponent(id)),
    };

    // SAFETY:
    // Valid entity must have valid table and row.
    match &cr.storage {
        Storage::SparseTag(_) => return Err(EcsError::IsTag(id)),
        Storage::SparseData(set) => Ok(set.contains(entity)),
        Storage::Tables(tables) => Ok(tables.contains_key(&r.table)),
    }
}
