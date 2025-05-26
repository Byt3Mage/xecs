use crate::{
    component::{Component, ensure_component},
    error::{EcsError, EcsResult},
    graph::table_traverse_add,
    id::Id,
    storage::{Storage, table::move_id},
    world::World,
};

/// Add the id as tag to the entity
///
/// # Safety
/// Caller ensures that id does not have associated data.
pub(crate) fn add_tag(world: &mut World, id: Id, tag: Id) -> EcsResult<()> {
    let r = world.id_index.get_record(id)?;

    // We cache the table and row here so if get record fails,
    // we don't create a component for id
    let src_table = r.table;
    let src_row = r.row;

    // Create ComponentRecord for tag if it doesn't exist.
    // Unlike components, tags can be registered on the fly,
    // allowing us to add regular entities as tags without first registering them.
    ensure_component(world, tag);

    let ci = world.components.get_mut(tag).unwrap();

    // SAFETY: we just checked that the id is a tag.
    match &mut ci.storage {
        Storage::SparseTag(set) => Ok(set.insert(id)),
        Storage::SparseData(_) => Err(EcsError::IsNotTag(tag)),
        Storage::Tables(_) => {
            if let Some(dst_table) = table_traverse_add(world, src_table, tag) {
                // SAFETY
                // - We ensured that dst_table is not the same as src.
                // - id is valid, which means that src_row must be valid.
                unsafe { move_id(world, id, src_table, src_row, dst_table) };
            }

            // Does nothing if there's no destination table.
            // This means that the id already contains the tag.
            Ok(())
        }
    }
}

/// Sets the value of a component for an id.
///
/// # Safety
/// - Caller must ensure that `val` is the same type and layout of the component.
pub(crate) unsafe fn set_component<C: Component>(
    world: &mut World,
    id: Id,
    comp: Id,
    val: C,
) -> Option<C> {
    let (table_id, row) = world.id_index.get_location(id).unwrap();
    let ci = world.components.get_mut(comp)?;

    // SAFETY:
    // - Valid entity must have valid table and row.
    // - Caller ensures that the type matches the component.
    match &mut ci.storage {
        Storage::SparseTag(_) => None,
        Storage::SparseData(set) => unsafe { set.insert(id, val) },
        Storage::Tables(_) => unsafe {
            let table = &mut world.table_index[table_id];

            match table.component_map.get(comp) {
                Some(&col) => Some(table.data.get_ptr_mut(row, col).replace(val)),
                None => {
                    let dst_table_id = table_traverse_add(world, table_id, comp).unwrap();

                    move_id(world, id, table_id, row, dst_table_id);

                    let table = &mut world.table_index[dst_table_id];
                    let col = *table.component_map.get(comp).unwrap();

                    table.data.push(col, val);
                    table.validate_data();

                    None
                }
            }
        },
    }
}

/// Sets the value of a component for an entity.
pub(crate) fn set_component_checked<C: Component>(
    world: &mut World,
    id: Id,
    comp: Id,
    val: C,
) -> Option<C> {
    let (table_id, row) = world.id_index.get_location(id).unwrap();

    ensure_component(world, comp);

    let ci = world.components.get_mut(comp)?;

    // Check that type matches.
    if let Some(ti) = &ci.type_info {
        if !ti.is::<C>() {
            return None;
        }
    }

    // SAFETY:
    // - Valid entity must have valid table and row.
    // - Caller ensures that the type matches the component.
    match &mut ci.storage {
        Storage::SparseTag(_) => None,
        Storage::SparseData(set) => unsafe { set.insert(id, val) },
        Storage::Tables(_) => unsafe {
            let table = &mut world.table_index[table_id];

            match table.component_map.get(comp) {
                Some(&col) => Some(table.data.get_ptr_mut(row, col).replace(val)),
                None => {
                    let dst_table_id = table_traverse_add(world, table_id, comp).unwrap();

                    move_id(world, id, table_id, row, dst_table_id);

                    let table = &mut world.table_index[dst_table_id];
                    let col = *table.component_map.get(comp).unwrap();

                    table.data.push(col, val);
                    table.validate_data();

                    None
                }
            }
        },
    }
}

pub(crate) fn has_component(world: &World, id: Id, comp: Id) -> bool {
    let (table, _) = match world.id_index.get_location(id) {
        Ok(r) => r,
        Err(_) => return false,
    };

    let cr = match world.components.get(comp) {
        Some(cr) => cr,
        None => return false,
    };

    // SAFETY: Valid id must have valid table and row.
    match &cr.storage {
        Storage::SparseTag(set) => set.contains(id),
        Storage::SparseData(set) => set.contains(id),
        Storage::Tables(tables) => tables.contains_key(&table),
    }
}
