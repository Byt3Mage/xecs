use crate::{
    component::{Component, ComponentBuilder, ComponentRecord, ComponentValue, Tag, TagBuilder},
    entity::Entity,
    entity_index::EntityIndex,
    error::{EcsError, EcsResult},
    storage::{
        Storage,
        sparse_set::SparseSet,
        table::move_entity_to_root,
        table_index::{TableId, TableIndex},
    },
    type_id::TypeImpl,
    world_utils::{add_tag, get_component_value, get_component_value_mut, set_component_value},
};
use const_assert::const_assert;
use std::ops::{Deref, DerefMut};

pub struct World {
    pub(crate) entity_index: EntityIndex,
    pub(crate) table_index: TableIndex,
    pub(crate) root_table: TableId,
    pub(crate) type_ids: Vec<Entity>,
    pub(crate) components: SparseSet<Entity, ComponentRecord>,
}

impl World {
    pub fn new() -> Self {
        // TODO: world initialization
        Self {
            entity_index: EntityIndex::new(),
            table_index: TableIndex::new(),
            root_table: TableId::NULL,
            type_ids: Vec::new(),
            components: SparseSet::new(),
        }
    }

    /// Gets the entity for the component type.
    #[inline(always)]
    pub fn id_t<T: 'static>(&mut self) -> EcsResult<Entity> {
        TypeImpl::<T>::id(self)
    }

    /// Gets a builder for registering the component or returns the id if already registered.
    #[inline(always)]
    pub fn component_t<C: ComponentValue>(&mut self) -> Result<ComponentBuilder<C>, Entity> {
        match TypeImpl::<C>::id(self) {
            Ok(id) => Err(id),
            Err(_) => Ok(ComponentBuilder::new(Entity::NULL)),
        }
    }

    /// Gets the registered component or returns a builder to create one from `id`.
    pub fn component(&mut self, id: Entity) -> Option<TagBuilder> {
        match self.components.get(&id) {
            Some(_) => None,
            None => Some(TagBuilder::new(id)),
        }
    }

    /// Returns a builder to create a new component.
    #[inline]
    pub fn new_component(&self) -> TagBuilder {
        TagBuilder::new(Entity::NULL)
    }

    pub fn new_entity(&mut self) -> Entity {
        let entity = self.entity_index.new_id();
        move_entity_to_root(self, entity);
        entity
    }

    /// Add the `tag` to entity.
    ///
    /// No side effect if the entity already contains the tag.
    #[inline(always)]
    pub fn add(&mut self, entity: Entity, tag: Tag) -> EcsResult<()> {
        add_tag(self, entity, tag.id())
    }

    /// Add the type as tag to entity.
    /// No side effect if entity already has tag.
    ///
    /// Component must be registered.
    ///
    /// Compilation fails if component is not ZST.
    pub fn add_t<T: ComponentValue>(&mut self, entity: Entity) -> EcsResult<()> {
        const_assert!(
            |T| std::mem::size_of::<T>() == 0,
            "can't use add_t for component, did you want to set?"
        );

        add_tag(self, entity, TypeImpl::<T>::id(self)?)
    }

    /// Checks if the entity has the component.
    pub fn has(&self, entity: Entity, id: Entity) -> EcsResult<bool> {
        let cr = self
            .components
            .get(&id)
            .ok_or(EcsError::UnregisteredComponent(id))?;
        let r = self.entity_index.get_record(entity)?;

        match &cr.storage {
            Storage::SparseTag(set) => Ok(set.has(&entity)),
            Storage::SparseData(set) => Ok(set.has(&entity)),
            Storage::Tables(tables) => {
                if id < Entity::HI_COMPONENT_ID {
                    if self.table_index[r.table].component_map_lo[id.as_usize()] >= 0 {
                        return Ok(true);
                    }
                }
                Ok(tables.contains_key(&r.table))
            }
        }
    }

    /// Checks if entity has the component.
    ///
    /// Returns `false` if the type is not registered
    /// or the entity does not have the type.
    pub fn has_t<C: ComponentValue>(&self, entity: Entity) -> EcsResult<bool> {
        self.has(entity, TypeImpl::<C>::id(self)?)
    }

    #[inline(always)]
    pub fn set<C: ComponentValue>(
        &mut self,
        entity: Entity,
        component: Component<C>,
        value: C,
    ) -> EcsResult<()> {
        set_component_value(self, entity, component.id(), value)
    }

    #[inline(always)]
    pub fn set_t<C: ComponentValue>(&mut self, entity: Entity, value: C) -> EcsResult<()> {
        set_component_value(self, entity, TypeImpl::<C>::id(self)?, value)
    }

    #[inline(always)]
    pub fn get<C: ComponentValue>(&self, entity: Entity, component: Component<C>) -> EcsResult<&C> {
        get_component_value(self, entity, component.id())
    }

    #[inline(always)]
    pub fn get_t<C: ComponentValue>(&self, entity: Entity) -> EcsResult<&C> {
        get_component_value(self, entity, TypeImpl::<C>::id(self)?)
    }

    #[inline(always)]
    pub fn get_mut<C: ComponentValue>(
        &mut self,
        entity: Entity,
        component: Component<C>,
    ) -> EcsResult<&mut C> {
        get_component_value_mut(self, entity, component.id())
    }

    #[inline(always)]
    pub fn get_mut_t<C: ComponentValue>(&mut self, entity: Entity) -> EcsResult<&mut C> {
        get_component_value_mut(self, entity, TypeImpl::<C>::id(self)?)
    }
}

pub struct WorldRef<'world> {
    world: &'world mut World,
}

impl Deref for WorldRef<'_> {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        self.world
    }
}

impl DerefMut for WorldRef<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world
    }
}

impl<'world> From<&'world mut World> for WorldRef<'world> {
    fn from(value: &'world mut World) -> Self {
        Self { world: value }
    }
}
