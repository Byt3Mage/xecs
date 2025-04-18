use crate::{entity_flags::EntityFlags, id::HI_COMPONENT_ID, storage::archetype::inc_traversable, world::World};

pub type Entity = u64;

/// Tag used to indicate an entity is a module.
pub const ECS_MODULE: Entity = HI_COMPONENT_ID + 4;
/// Tag used to indicate an entity is private to a module.
pub const ECS_PRIVATE: Entity = HI_COMPONENT_ID + 5;
/// Tag used to indicate an entity is a prefab.
pub const ECS_PREFAB: Entity = HI_COMPONENT_ID + 6;
/// When added to an entity, the tag is skipped by queries,
/// unless DISABLED is explicitly queried for.
pub const ECS_DISABLED: Entity = HI_COMPONENT_ID + 7;
/// Tag used for entities that should never be returned by queries.
/// Used for entities that have special meaning to the query engine.
pub const ECS_NOT_QUERYABLE: Entity = HI_COMPONENT_ID + 8;

/// Used for slots in prefabs.
pub const ECS_SLOT_OF: Entity = HI_COMPONENT_ID + 9;
/// Used to track entities used with id flags.
pub const ECS_FLAG: Entity = HI_COMPONENT_ID + 10;

/* Marker entities for query encoding */
pub const ECS_WILDCARD: Entity = HI_COMPONENT_ID + 11;
pub const ECS_ANY: Entity = HI_COMPONENT_ID + 12;
pub const ECS_THIS: Entity = HI_COMPONENT_ID + 13;
pub const ECS_VARIABLE: Entity = HI_COMPONENT_ID + 14;

/* Builtin relationships */
pub const ECS_CHILD_OF: Entity = HI_COMPONENT_ID + 34;
pub const ECS_IS_A: Entity = HI_COMPONENT_ID + 35;
pub const ECS_DEPENDS_ON: Entity = HI_COMPONENT_ID + 36;

#[macro_export]
macro_rules! record_add_flag {
    ($world: expr, $record: expr, $flag: expr) => {
        {
            
        }
    };
}

pub(crate) fn add_flag(world: &mut World, entity: Entity, flag: EntityFlags) {
    let record= world.entity_index.get_record_mut(entity).unwrap();
    
    if flag == EntityFlags::IS_TRAVERSABLE && !record.flags.contains(flag) {
        let arch = world.archetypes.get_mut(record.arch).unwrap();
        inc_traversable(arch, 1);
    }

    record.flags |= flag;
}