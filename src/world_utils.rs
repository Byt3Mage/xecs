use crate::{component::ComponentValue, entity::Entity, error::EcsResult, graph::archetype_traverse_add, id::Id, storage::archetype::move_entity, world::World};

pub(crate) fn set_component_value<C: ComponentValue>(world: &mut World, entity: Entity, id: Id, value: C) -> EcsResult<()> {
    let (arch_id, row) = world.entity_index.get_location(entity)?;

    // TODO: optimize.
    let cr = world.components.get(&id).unwrap();

    if let Some(cl) = cr.archetypes.get(&arch_id) {
        let col = cl.column_index.expect("INTERNAL ERROR: no column for id, did you mean to add?");
        let arch = world.archetypes.get_mut(arch_id).unwrap();
        unsafe { arch.data.set_unchecked(col, row, value); }

        return Ok(())
    }
    
    let dst_id = archetype_traverse_add(world, arch_id, id);

    debug_assert!(arch_id != dst_id);

    // SAFETY:
    // - row is valid in enitity index.
    // - archetype does not contain id, so dst_id must be different.
    unsafe {
        let row = move_entity(world, entity, arch_id, row, dst_id);

        let cr = world.components.get(&id).expect("INTERNAL ERROR: component not registered");
        let cl = cr.archetypes.get(&dst_id).expect("INTERNAL ERROR: id not in archetype");
        let col = cl.column_index.expect("INTERNAL ERROR: no column for id, did you mean to add?");
    
        let dst_arch = world.archetypes.get_mut(dst_id).unwrap();
      
        dst_arch.data.set_unchecked(col, row, value);

        Ok(())
    }       
}