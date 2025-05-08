use std::any::TypeId;

use crate::{
    component::{Component, UntypedComponentDesc},
    entity::Entity,
    error::{EcsError, EcsResult},
    graph::table_traverse_add,
    storage::{Storage, StorageType, table::move_entity},
    world::World,
};
use const_assert::const_assert;

pub(crate) fn register_type<T: 'static>(world: &mut World) -> Entity {
    debug_assert!(!world.type_map.contains::<T>(), "type already registered");
    let new_id = world.new_entity();
    world.type_map.insert::<T>(new_id);
    new_id
}

/// Add the id as tag to the entity
///
/// # Safety
/// Caller ensures that id does not have associated data.
pub(crate) fn add_tag(world: &mut World, entity: Entity, id: Entity) -> EcsResult<()> {
    let r = world.entity_index.get_record(entity)?;

    // Create ComponentRecord for tag if it doesn't exist.
    // Unlike components, tags can be registered on the fly,
    // allowing us to add regular entities as tags without first registering them.
    if !world.components.contains(&id) {
        assert!(world.entity_index.is_alive(id));
        // TODO: default flags?
        let cr = UntypedComponentDesc::new()
            .storage(StorageType::Tables)
            .build(id);
        world.components.insert(id, cr);
    }

    // Unwrap should never fail here, since we just ensured the component exists.
    let cr = world.components.get_mut(&id).unwrap();

    debug_assert!(cr.type_info.is_none(), "id has data, can't use add");

    match &mut cr.storage {
        Storage::SparseTag(tag) => Ok(tag.insert(entity)),
        Storage::SparseData(_) => Err(EcsError::IsNotTag(id)),
        Storage::Tables(_) => {
            let src_table = r.table;
            let src_row = r.row;

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
///
/// # Safety
/// Caller ensures that the type matches the id.
pub(crate) fn set_component_value<C: Component>(
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

    // SAFETY: Valid entity must have valid table and row.
    match &mut cr.storage {
        Storage::SparseTag(_) => Err(EcsError::IsTag(id)),
        Storage::SparseData(set) => Ok(set.insert(entity, value)),
        Storage::Tables(_) => {
            let src_table = r.table;
            let src_row = r.row;

            if let Some(dst_table) = table_traverse_add(world, src_table, id) {
                // SAFETY
                // - We ensured that dst_table is not the same as src.
                // - entity is valid, which means that src_row must be valid.
                unsafe {
                    let dst_row = move_entity(world, entity, src_table, src_row, dst_table);
                    world.table_index[dst_table].set_uninit(dst_row, id, value);
                }
            } else {
                // No archetype move, replace the value in the current archetype.

                // SAFETY:
                // - row is obtained from a valid entity.
                // - type is confirmed to be valid for component id.
                // - since there is no table move, src_row is already initialized,
                //   which means we use `set` to replace the value it contained.
                unsafe { world.table_index[src_table].set(src_row, id, value) };
            }

            // Does nothing if there's no destination table.
            // This means that the entity already contains the tag.
            Ok(())
        }
    }
}

pub(crate) fn get_component_value<C: Component>(
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

    // SAFETY: Valid entity must have valid table and row.
    let ptr = match &cr.storage {
        Storage::SparseTag(_) => return Err(EcsError::IsTag(id)),
        Storage::SparseData(set) => set.get(entity),
        Storage::Tables(_) => unsafe { world.table_index[r.table].get(entity, r.row, id) },
    }?;

    if ptr.type_id == TypeId::of::<C>() {
        // SAFETY:
        // - ptr is NonNull
        // - we just checked that the type matches.
        Ok(unsafe { ptr.ptr.cast().as_ref() })
    } else {
        Err(EcsError::TypeMismatch {
            exp: ptr.type_name,
            got: std::any::type_name::<C>(),
        })
    }
}

pub(crate) fn get_component_value_mut<C: Component>(
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

    // SAFETY: Valid entity must have valid row.
    let ptr = match &mut cr.storage {
        Storage::SparseTag(_) => return Err(EcsError::IsTag(id)),
        Storage::SparseData(set) => set.get_mut(entity),
        Storage::Tables(_) => unsafe { world.table_index[r.table].get_mut(entity, r.row, id) },
    }?;

    if ptr.type_id == TypeId::of::<C>() {
        // SAFETY:
        // - ptr is NonNull
        // - we just checked that the type matches.
        Ok(unsafe { ptr.ptr.cast().as_mut() })
    } else {
        Err(EcsError::TypeMismatch {
            exp: ptr.type_name,
            got: std::any::type_name::<C>(),
        })
    }
}
