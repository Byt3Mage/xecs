use crate::{entity::Entity, entity_index::EntityLocation, graph::archetype_traverse_add, id::Id, pointer::PtrMut, storage::archetype::move_entity, world::World};

pub(crate) fn ensure_component_ptr(world: &mut World, entity: Entity, location: EntityLocation, id: Id) -> PtrMut {
    let src_arch = location.arch;
    let dst_arch = archetype_traverse_add(world,src_arch, id);

    if src_arch == dst_arch {
        let cr = world.components.get(&id).expect("INTERNAL ERROR: component not registered");
        let cl = cr.archetypes.get(&src_arch).expect("INTERNAL ERROR: id not in archetype");
        let column = cl.column_index.expect("INTERNAL ERROR: no column for id, did you mean to add?");

        let src_arch = world.archetypes.get_mut(src_arch).unwrap();
        
        unsafe { src_arch.data.get_unchecked_mut(column, location.row) }
    }
    else {
        // SAFETY:
        // - src_row is valid in enitity index.
            // - we just checked that src_arch and dst_arch are not the same.
        unsafe {
                move_entity(world, entity, src_arch, location.row, dst_arch)
        }

        let cr = world.components.get(&id).expect("INTERNAL ERROR: component not registered");
        let cl = cr.archetypes.get(&dst_arch).expect("INTERNAL ERROR: id not in archetype");
        let column = cl.column_index.expect("INTERNAL ERROR: no column for id, did you mean to add?");

        let dst_arch = world.archetypes.get_mut(src_arch).unwrap();
        
        unsafe { dst_arch.data.get_unchecked_mut(column, location.row) }
    }
}