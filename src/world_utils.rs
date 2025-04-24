use crate::{
    component::ComponentValue,
    entity::Entity,
    error::{EcsError, EcsResult},
    id::{HI_COMPONENT_ID, Id},
    world::World,
};

pub(crate) fn commit(world: &mut World) {
    // TODO: implement commit logic
}

pub(crate) fn set_component_value<C: ComponentValue>(
    world: &mut World,
    entity: Entity,
    id: Id,
    value: C,
) -> EcsResult<()> {
    let record = world.entity_index.get_record_mut(entity)?;
    debug_assert!(record.table.is_some(), "valid entity must have a table");

    let mut table = unsafe { record.table.unwrap_unchecked() };

    todo!()
}

pub(crate) fn get_component_data<C: ComponentValue>(
    world: &World,
    entity: Entity,
    id: Id,
) -> EcsResult<&C> {
    let record = world.entity_index.get_record(entity)?;
    debug_assert!(record.table.is_some(), "valid entity must have a table");

    unsafe {
        let table = record.table.unwrap_unchecked().as_ref();

        if id < HI_COMPONENT_ID {
            let column = *table.component_map_lo.get_unchecked(id as usize);
            if column >= 0 {
                let ptr = table.data.get_unchecked(column as usize, record.row);
                return Ok(ptr.deref());
            }
        }

        let Some(column) = table.component_map_hi.get(&id) else {
            return Err(EcsError::Component("data not found for component"));
        };

        // SAFETY:
        // colum is valid in table.
        // row is valid in entity index.
        let ptr = table.data.get_unchecked(*column, record.row);
        Ok(ptr.deref())
    }
}

// TODO: ensure callers perform type checks
pub(crate) fn get_component_data_mut<C: ComponentValue>(
    world: &mut World,
    entity: Entity,
    id: Id,
) -> EcsResult<&mut C> {
    let record = world.entity_index.get_record_mut(entity)?;
    debug_assert!(record.table.is_some(), "valid entity must have a table");

    unsafe {
        let table = record.table.unwrap_unchecked().as_mut();

        if id < HI_COMPONENT_ID {
            let column = *table.component_map_lo.get_unchecked(id as usize);
            if column >= 0 {
                let ptr = table.data.get_unchecked_mut(column as usize, record.row);
                return Ok(ptr.deref_mut());
            }
        }

        let Some(column) = table.component_map_hi.get(&id) else {
            return Err(EcsError::Component("data not found for component"));
        };

        // SAFETY:
        // colum is valid in table.
        // row is valid in entity index.
        let ptr = table.data.get_unchecked_mut(*column, record.row);
        Ok(ptr.deref_mut())
    }
}
