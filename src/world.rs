use crate::{
    component::{
        self, Component, ComponentDesc, ComponentRecord, ComponentValue, Tag, TypedEntity,
        UntypedComponentDesc,
    },
    entity::{Entity, EntityMap},
    entity_index::EntityIndex,
    error::{EcsError, EcsResult, unregistered_type_err},
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
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::slice_from_raw_parts,
};

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
    pub fn id_t<C: ComponentValue>(&self) -> Option<TypedEntity<C>> {
        TypedEntity::new(self)
    }

    /// Registers the type with the world or returns its id if already registered.
    #[inline(always)]
    pub fn component_t<C: ComponentValue>(
        &mut self,
        f: impl FnOnce() -> ComponentDesc<C>,
    ) -> TypedEntity<C> {
        let id = match self.type_map.get::<C>() {
            Some(&id) => {
                if self.components.contains(&id) {
                    // early out if component is registered
                    return TypedEntity {
                        id,
                        _marker: PhantomData,
                    };
                }
                id
            }
            // Register type if not registered.
            None => register_type::<C>(self),
        };

        self.components.insert(id, f().build(id));

        TypedEntity {
            id,
            _marker: PhantomData,
        }
    }

    /// Gets or creates a component from this id.
    /// Returns `None` if a component already exists for this id
    /// and the type is mismatched
    #[inline(always)]
    pub fn to_component_t<C: ComponentValue>(
        &mut self,
        id: Entity,
        f: impl FnOnce() -> ComponentDesc<C>,
    ) -> Option<Component<C>> {
        const_assert!(
            |C| size_of::<C>() != 0,
            "can't convert entity to ZST component, use to_component instead"
        );
        match self.components.get(&id) {
            Some(cr) => match &cr.type_info {
                Some(ti) if ti.is::<C>() => Some(Component {
                    id,
                    _marker: PhantomData,
                }),
                _ => None,
            },
            None => {
                self.components.insert(id, f().build(id));
                Some(Component {
                    id,
                    _marker: PhantomData,
                })
            }
        }
    }

    /// Gets the registered component or calls f to build one.
    ///
    /// The builder is lazily evaluated and only called if the id does not already have a component.
    pub fn to_component(&mut self, id: Entity, f: impl FnOnce() -> UntypedComponentDesc) {
        if !self.components.contains(&id) {
            self.components.insert(id, f().build(id))
        }
    }

    pub fn new_component_t<C: ComponentValue>(
        &mut self,
        builder: ComponentDesc<C>,
    ) -> Component<C> {
        const_assert!(
            |C| size_of::<C>() != 0,
            "can't create new component from ZST, use new_component instead"
        );
        let id = self.new_entity();
        self.components.insert(id, builder.build(id));
        Component {
            id,
            _marker: PhantomData,
        }
    }

    pub fn new_component(&mut self, builder: UntypedComponentDesc) -> Entity {
        let id = self.new_entity();
        self.components.insert(id, builder.build(id));
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
    pub fn add(&mut self, entity: Entity, tag: impl Into<Tag>) -> EcsResult<()> {
        add_tag(self, entity, tag.into().0)
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

        let id = match self.type_map.get::<C>() {
            Some(&id) => id,
            None => return unregistered_type_err::<C, _>(),
        };

        add_tag(self, entity, id)
    }

    /// Checks if the entity has the component.
    pub fn has(&self, entity: Entity, id: impl Into<Entity>) -> EcsResult<bool> {
        let id = id.into();
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
    pub fn has_t<C: ComponentValue>(&self, entity: Entity) -> EcsResult<bool> {
        let id = match self.type_map.get::<C>() {
            Some(&id) => id,
            None => return unregistered_type_err::<C, _>(),
        };

        self.has(entity, id)
    }

    #[inline(always)]
    pub fn set<C: ComponentValue>(
        &mut self,
        entity: Entity,
        component: impl Into<Component<C>>,
        val: C,
    ) -> EcsResult<()> {
        set_component_value(self, entity, component.into().id, val)
    }

    #[inline(always)]
    pub fn set_t<C: ComponentValue>(&mut self, entity: Entity, val: C) -> EcsResult<()> {
        let id = match self.type_map.get::<C>() {
            Some(&id) => id,
            None => return unregistered_type_err::<C, _>(),
        };

        set_component_value(self, entity, id, val)
    }

    #[inline(always)]
    pub fn get<C: ComponentValue>(
        &self,
        entity: Entity,
        component: impl Into<Component<C>>,
    ) -> EcsResult<&C> {
        get_component_value(self, entity, component.into().id)
    }

    #[inline(always)]
    pub fn get_t<C: ComponentValue>(&self, entity: Entity) -> EcsResult<&C> {
        let id = match self.type_map.get::<C>() {
            Some(&id) => id,
            None => return unregistered_type_err::<C, _>(),
        };

        get_component_value(self, entity, id)
    }

    #[inline(always)]
    pub fn get_mut<C: ComponentValue>(
        &mut self,
        entity: Entity,
        component: impl Into<Component<C>>,
    ) -> EcsResult<&mut C> {
        get_component_value_mut(self, entity, component.into().id)
    }

    #[inline(always)]
    pub fn get_mut_t<C: ComponentValue>(&mut self, entity: Entity) -> EcsResult<&mut C> {
        let id = match self.type_map.get::<C>() {
            Some(&id) => id,
            None => return unregistered_type_err::<C, _>(),
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
