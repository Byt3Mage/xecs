use std::any::TypeId;

use crate::{component::ComponentValue, entity::Entity, error::{type_mismatch_err, EcsError, EcsResult}, graph::archetype_traverse_add, id::Id, storage::archetype::move_entity, world::World};

pub(crate) fn set_component_value<C: ComponentValue>(world: &mut World, entity: Entity, id: Id, value: C) -> EcsResult<()> {
    match world.type_index.get(id) {
        Some(ti) => if ti.type_id != TypeId::of::<C>() { return type_mismatch_err(); },
        None => return Err(EcsError::Component("can't use set for tag, use add instead")),
    }

    let (arch_id, row) = world.entity_index.get_location(entity)?;
    let arch = world.archetypes.get_mut(arch_id).unwrap();

    // TODO: profile if hashmap is okay.
    if let Some(&column) = arch.component_map.get(&id) {
        unsafe {
            let ptr = arch.data.get_unchecked_mut(column, row).as_ptr().cast::<C>();
            let _ = ptr.read(); // drop the previous value.
            ptr.write(value);
        }
        return Ok(());
    }
    
    // current archetype does not contain the component, 
    // so we get or create an archetype with the component and move the entity to it.
    let dst_id = archetype_traverse_add(world, arch_id, id);

    debug_assert!(arch_id != dst_id);

    // SAFETY:
    // - row is valid in enitity index.
    // - archetype does not contain id, so dst_id must be different.
    unsafe {
        let row = move_entity(world, entity, arch_id, row, dst_id);
        let arch = world.archetypes.get_mut(dst_id).unwrap();
        let column = arch.component_map.get(&id).unwrap(); // TODO: profile if hashmap is okay.
        let ptr = arch.data.get_unchecked_mut(*column, row).as_ptr().cast::<C>();
        //let _ = ptr.read(); //we don't read out of the pointer since the row is newly added.
        ptr.write(value);
        return Ok(());
    }
}

pub(crate) fn get_component_value<C: ComponentValue>(world: &World, entity: Entity, id: Id) -> EcsResult<&C> {
    match world.type_index.get(id) {
        Some(ti) => if ti.type_id != TypeId::of::<C>() { return type_mismatch_err(); },
        None => return Err(EcsError::Component("can't use get for tag, use has instead")),
    }

    let (arch, row) = world.entity_index.get_location(entity)?;
    let arch = world.archetypes.get(arch).expect("INTERNAL ERROR: live entity must have an archetype");
    let column = arch.component_map.get(&id).ok_or(EcsError::Component("Entity does not have component"))?;

    // SAFETY: 
    // colum is valid in archetype.
    // row is valid in entity index.
    unsafe {
        let ptr = arch.data.get_unchecked(*column, row);
        Ok(ptr.deref())
    }
}

pub(crate) fn get_component_value_mut<C: ComponentValue>(world: &mut World, entity: Entity, id: Id) -> EcsResult<&mut C> {
    match world.type_index.get(id) {
        Some(ti) => if ti.type_id != TypeId::of::<C>() { return type_mismatch_err(); },
        None => return Err(EcsError::Component("can't use get for tag, use has instead")),
    }

    let (arch, row) = world.entity_index.get_location(entity)?;
    let arch = world.archetypes.get_mut(arch).expect("INTERNAL ERROR: live entity must have an archetype");
    let column = arch.component_map.get(&id).ok_or(EcsError::Component("Entity does not have component"))?;

    // SAFETY: 
    // colum is valid in archetype.
    // row is valid in entity index.
    unsafe {
        let ptr = arch.data.get_unchecked_mut(*column, row);
        Ok(ptr.deref_mut())
    }
}