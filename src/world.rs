use crate::{
    component::{Component, ComponentBuilder, ComponentRecord, ComponentValue, Tag, TagBuilder},
    entity::Entity,
    entity_index::EntityIndex,
    error::{EcsError, EcsResult},
    flags::TableFlags,
    graph::GraphNode,
    storage::{
        Storage,
        sparse_set::SparseSet,
        table::{Table, move_entity_to_root},
        table_data::TableData,
        table_index::{TableId, TableIndex},
    },
    type_impl::TypeImpl,
    type_info::Type,
    world_utils::{add_tag, get_component_value, get_component_value_mut, set_component_value},
};
use const_assert::const_assert;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

pub struct World {
    pub(crate) entity_index: EntityIndex,
    pub(crate) table_index: TableIndex,
    pub(crate) root_table: TableId,
    pub(crate) type_ids: Vec<Entity>,
    pub(crate) components: SparseSet<Entity, ComponentRecord>,
}

impl World {
    pub fn new() -> Self {
        let mut table_index = TableIndex::new();
        let root_table = table_index.add_with_id(|id| Table {
            id,
            flags: TableFlags::empty(),
            type_: Type::from(vec![]),
            data: TableData::new(Box::from([])),
            component_map_lo: [-1; Entity::HI_COMPONENT_ID.as_usize()],
            component_map_hi: HashMap::new(),
            node: GraphNode::new(),
        });

        Self {
            entity_index: EntityIndex::new(),
            table_index,
            root_table,
            type_ids: Vec::new(),
            components: SparseSet::new(),
        }
    }

    /// Gets the entity for the component type.
    #[inline(always)]
    pub fn id_t<T: TypeImpl>(&mut self) -> EcsResult<Entity> {
        T::id(self)
    }

    /// Gets a builder for registering the component or returns the id if already registered.
    #[inline(always)]
    pub fn component_t<C: ComponentValue>(&mut self) -> Result<ComponentBuilder<C>, Entity> {
        match C::id(self) {
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
    pub fn add_t<C: ComponentValue>(&mut self, entity: Entity) -> EcsResult<()> {
        const_assert!(
            |C| std::mem::size_of::<C>() == 0,
            "can't use add_t for component, did you want to set?"
        );

        add_tag(self, entity, C::id(self)?)
    }

    /// Checks if the entity has the component.
    pub fn has(&self, entity: Entity, id: Entity) -> EcsResult<bool> {
        let Some(cr) = self.components.get(&id) else {
            return Err(EcsError::UnregisteredComponent(id));
        };
        let r = self.entity_index.get_record(entity)?;

        let has = match &cr.storage {
            Storage::SparseTag(set) => set.has(entity),
            Storage::SparseData(set) => set.has(entity),
            Storage::Tables(tables) => {
                if id < Entity::HI_COMPONENT_ID {
                    if self.table_index[r.table].component_map_lo[id.as_usize()] >= 0 {
                        return Ok(true);
                    }
                }

                tables.contains_key(&r.table)
            }
        };

        Ok(has)
    }

    /// Checks if entity has the component.
    ///
    /// Returns `false` if the type is not registered
    /// or the entity does not have the type.
    pub fn has_t<C: ComponentValue>(&self, entity: Entity) -> EcsResult<bool> {
        self.has(entity, C::id(self)?)
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
        set_component_value(self, entity, C::id(self)?, value)
    }

    #[inline(always)]
    pub fn get<C: ComponentValue>(&self, entity: Entity, component: Component<C>) -> EcsResult<&C> {
        get_component_value(self, entity, component.id())
    }

    #[inline(always)]
    pub fn get_t<C: ComponentValue>(&self, entity: Entity) -> EcsResult<&C> {
        get_component_value(self, entity, C::id(self)?)
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
        get_component_value_mut(self, entity, C::id(self)?)
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
