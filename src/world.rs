use crate::{
    component::{Component, ComponentDesc, ComponentRecord, UntypedComponentDesc},
    entity::{Entity, EntityMap},
    entity_index::EntityIndex,
    error::{EcsError, EcsResult, unregistered_type},
    flags::TableFlags,
    graph::GraphNode,
    storage::{
        Storage,
        sparse_set::SparseSet,
        table::{Table, move_entity_to_root},
        table_data::TableData,
    },
    table_index::{TableId, TableIndex},
    types::{IdList, TypeMap},
    world_utils::{
        add_tag, get_component_value, get_component_value_mut, register_type, set_component_value,
    },
};
use const_assert::const_assert;
use std::ops::{Deref, DerefMut};

pub struct World {
    pub(crate) entity_index: EntityIndex,
    pub(crate) table_index: TableIndex,
    pub(crate) root_table: TableId,
    pub(crate) type_map: TypeMap<Entity>,
    pub(crate) components: SparseSet<Entity, ComponentRecord>,
}

impl World {
    pub fn new() -> Self {
        let mut table_index = TableIndex::new();
        let root_table = table_index.add_with_id(|id| Table {
            id,
            flags: TableFlags::empty(),
            ids: IdList::from(vec![]),
            data: TableData::new(Box::from([])),
            component_map: EntityMap::default(),
            node: GraphNode::new(),
        });

        Self {
            entity_index: EntityIndex::new(),
            table_index,
            root_table,
            type_map: TypeMap::new(),
            components: SparseSet::new(),
        }
    }

    /// Gets the entity id for the type.
    /// Returns `None` if type is not registered with this world.
    #[inline(always)]
    pub fn id_t<C: Component>(&self) -> Option<Entity> {
        self.type_map.get::<C>().copied()
    }

    /// Registers the type with the world if it isn't and returns its id.
    ///
    /// This function eagerly evaluates `desc` (see [World::register_with]
    /// for lazily evaluated descriptor).
    pub fn register<C: Component>(&mut self, desc: ComponentDesc<C>) -> Entity {
        let id = match self.type_map.get::<C>() {
            Some(&id) => {
                if self.components.contains(&id) {
                    // early out if component is registered
                    return id;
                }

                id
            }
            // Register type if not registered.
            None => register_type::<C>(self),
        };
        self.components.insert(id, desc.build(id));
        id
    }

    /// Registers the type with the world or returns its id if already registered.
    ///
    /// Lazily evaluates the descriptor and only calls it if the type is not registered.
    pub fn register_with<C: Component>(&mut self, f: impl Fn() -> ComponentDesc<C>) -> Entity {
        let id = match self.type_map.get::<C>() {
            Some(&id) => {
                if self.components.contains(&id) {
                    // early out if component is registered
                    return id;
                }

                id
            }
            // Register type if not registered.
            None => register_type::<C>(self),
        };
        self.components.insert(id, f().build(id));
        id
    }

    /// Creates a component from this `id` if one doesn't exist.
    /// Returns `false` if the component already exists.
    #[inline(always)]
    pub fn to_component_t<C>(&mut self, id: Entity, f: impl FnOnce() -> ComponentDesc<C>) -> bool
    where
        C: Component,
    {
        match self.components.get(&id) {
            Some(_) => false,
            None => {
                self.components.insert(id, f().build(id));
                true
            }
        }
    }

    /// Creates a component from this `id` if one doesn't exist.
    /// Returns `false` if the component already exists.
    #[inline(always)]
    pub fn to_component(&mut self, id: Entity, f: impl FnOnce() -> UntypedComponentDesc) -> bool {
        match self.components.get(&id) {
            Some(_) => false,
            None => {
                self.components.insert(id, f().build(id));
                true
            }
        }
    }

    /// Creates a new entity id and assigns a component to it.
    ///
    /// Useful creating "newtype" components.
    pub fn new_component_t<C: Component>(&mut self, desc: ComponentDesc<C>) -> Entity {
        let id = self.new_entity();
        self.components.insert(id, desc.build(id));
        id
    }

    /// Creates a new entity id and assigns a component to it.
    ///
    /// Useful creating "newtype" components.
    pub fn new_component(&mut self, desc: UntypedComponentDesc) -> Entity {
        let id = self.new_entity();
        self.components.insert(id, desc.build(id));
        id
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
    pub fn add(&mut self, entity: Entity, id: Entity) -> EcsResult<()> {
        add_tag(self, entity, id)
    }

    /// Add the type as tag to entity.
    /// No side effect if entity already has tag.
    ///
    /// Component must be registered.
    ///
    /// Compilation fails if component is not ZST.
    pub fn add_t<C: Component>(&mut self, entity: Entity) -> EcsResult<()> {
        const_assert!(
            |C| std::mem::size_of::<C>() == 0,
            "can't use add_t for component, did you want to set?"
        );

        let id = match self.type_map.get::<C>() {
            Some(&id) => id,
            None => return Err(unregistered_type::<C>()),
        };

        add_tag(self, entity, id)
    }

    /// Checks if the entity has the component.
    pub fn has(&self, entity: Entity, id: Entity) -> EcsResult<bool> {
        let cr = match self.components.get(&id) {
            Some(cr) => cr,
            None => return Err(EcsError::UnregisteredComponent(id)),
        };
        let r = self.entity_index.get_record(entity)?;

        let has = match &cr.storage {
            Storage::SparseTag(set) => set.has(entity),
            Storage::SparseData(set) => set.has(entity),
            Storage::Tables(tables) => tables.contains_key(&r.table),
        };

        Ok(has)
    }

    /// Checks if entity has the component.
    ///
    /// Returns `false` if the type is not registered
    /// or the entity does not have the type.
    pub fn has_t<C: Component>(&self, entity: Entity) -> EcsResult<bool> {
        let id = match self.type_map.get::<C>() {
            Some(&id) => id,
            None => return Err(unregistered_type::<C>()),
        };

        self.has(entity, id)
    }

    #[inline(always)]
    pub fn set<C: Component>(&mut self, entity: Entity, id: Entity, val: C) -> EcsResult<()> {
        set_component_value(self, entity, id, val)
    }

    #[inline(always)]
    pub fn set_t<C: Component>(&mut self, entity: Entity, val: C) -> EcsResult<()> {
        let id = match self.type_map.get::<C>() {
            Some(&id) => id,
            None => return Err(unregistered_type::<C>()),
        };

        set_component_value(self, entity, id, val)
    }

    #[inline(always)]
    pub fn get<C: Component>(&self, entity: Entity, id: Entity) -> EcsResult<&C> {
        get_component_value(self, entity, id)
    }

    #[inline(always)]
    pub fn get_t<C: Component>(&self, entity: Entity) -> EcsResult<&C> {
        let id = match self.type_map.get::<C>() {
            Some(&id) => id,
            None => return Err(unregistered_type::<C>()),
        };

        get_component_value(self, entity, id)
    }

    #[inline(always)]
    pub fn get_mut<C: Component>(&mut self, entity: Entity, id: Entity) -> EcsResult<&mut C> {
        get_component_value_mut(self, entity, id)
    }

    #[inline(always)]
    pub fn get_mut_t<C: Component>(&mut self, entity: Entity) -> EcsResult<&mut C> {
        let id = match self.type_map.get::<C>() {
            Some(&id) => id,
            None => return Err(unregistered_type::<C>()),
        };

        get_component_value_mut(self, entity, id)
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
