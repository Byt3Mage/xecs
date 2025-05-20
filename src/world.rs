use crate::{
    component::{Component, ComponentDesc, ComponentInfo, TagDesc},
    error::{EcsResult, unregistered_type},
    flags::TableFlags,
    graph::GraphNode,
    id::{Id, IdMap, id_index::IdIndex},
    pointer::{Ptr, PtrMut},
    storage::{
        table::{Table, move_entity_to_root},
        table_data::TableData,
    },
    table_index::{TableId, TableIndex},
    types::{IdList, TypeMap},
    world_utils::{add_tag, get_component, get_component_mut, has_component, set_component},
};
use const_assert::const_assert;
use std::ops::{Deref, DerefMut};

pub struct World {
    pub(crate) id_index: IdIndex,
    pub(crate) type_map: TypeMap<Id>,
    pub(crate) components: IdMap<ComponentInfo>,
    pub(crate) table_index: TableIndex,
    pub(crate) root_table: TableId,
}

impl World {
    pub fn new() -> Self {
        let mut table_index = TableIndex::new();
        let root_table = table_index.add_with_id(|id| Table {
            id,
            _flags: TableFlags::empty(),
            ids: IdList::from(vec![]),
            data: TableData::new(Box::from([])),
            component_map: IdMap::new(),
            node: GraphNode::new(),
        });

        Self {
            id_index: IdIndex::new(),
            type_map: TypeMap::new(),
            components: IdMap::new(),
            table_index,
            root_table,
        }
    }

    /// Gets the entity id for the type.
    /// Returns `None` if type is not registered with this world.
    #[inline(always)]
    pub fn id_t<C: Component>(&self) -> Option<Id> {
        self.type_map.get::<C>().copied()
    }

    /// Registers the type with the world if it isn't and returns its id.
    ///
    /// This function eagerly evaluates `desc` (see [World::register_with]
    /// for lazily evaluated descriptor).
    pub fn register<C: Component>(&mut self, desc: ComponentDesc<C>) -> Id {
        let id = match self.type_map.get::<C>() {
            Some(&id) => {
                if self.components.contains(id) {
                    // early out if component is registered
                    return id;
                }
                id
            }
            // Register type if not registered.
            None => {
                let new_id = self.new_id();
                self.type_map.insert::<C>(new_id);
                new_id
            }
        };

        desc.build(self, id);
        id
    }

    /// Registers the type with the world or returns its id if already registered.
    ///
    /// Lazily evaluates the descriptor and only calls it if the type is not registered.
    pub fn register_with<C: Component>(&mut self, f: impl Fn() -> ComponentDesc<C>) -> Id {
        let id = match self.type_map.get::<C>() {
            Some(&id) => {
                if self.components.contains(id) {
                    // early out if component is registered
                    return id;
                }

                id
            }
            // Register type if not registered.
            None => {
                let new_id = self.new_id();
                self.type_map.insert::<C>(new_id);
                new_id
            }
        };

        f().build(self, id);
        id
    }

    /// Creates a component from this `id` if one doesn't exist.
    ///
    /// Returns `false` if:
    /// - `id` is already a component/tag.
    /// - `id` is a pair.
    /// - `id` is not valid.
    #[inline(always)]
    pub fn to_component<C>(&mut self, id: Id, f: impl FnOnce() -> ComponentDesc<C>) -> bool
    where
        C: Component,
    {
        if !self.id_index.is_alive(id) {
            return false;
        }

        match self.components.get(id) {
            Some(_) => false,
            None => {
                if id.is_pair() {
                    false
                } else {
                    f().build(self, id);
                    true
                }
            }
        }
    }

    /// Creates a tag from this `id` if one doesn't exist.
    ///
    /// Returns `false` if:
    /// - `id` is already a component/tag.
    /// - `id` is a pair.
    /// - `id` is not valid.
    #[inline(always)]
    pub fn to_tag(&mut self, id: Id, f: impl FnOnce() -> TagDesc) -> bool {
        if !self.id_index.is_alive(id) {
            return false;
        }

        match self.components.get(id) {
            Some(_) => false,
            None => {
                if id.is_pair() {
                    false
                } else {
                    f().build(self, id);
                    true
                }
            }
        }
    }

    /// Creates a new entity id and assigns a component to it.
    ///
    /// Useful for creating "newtype" components.
    pub fn new_component<C: Component>(&mut self, desc: ComponentDesc<C>) -> Id {
        const_assert!(|C| size_of::<C>() != 0, "can't use new_component for ZST");

        let id = self.new_id();
        desc.build(self, id);
        id
    }

    /// Creates a new entity id and assigns a component to it.
    pub fn new_tag(&mut self, desc: TagDesc) -> Id {
        let id = self.new_id();
        desc.build(self, id);
        id
    }

    pub fn new_id(&mut self) -> Id {
        let id = self.id_index.new_id();
        move_entity_to_root(self, id);
        id
    }

    /// Add `id` as tag to entity. No side effect if entity already has tag.
    #[inline]
    pub fn add_id(&mut self, entity: Id, id: impl Into<Id>) -> EcsResult<()> {
        add_tag(self, entity, id.into())
    }

    /// Add the type as tag to entity. No side effect if entity already has tag.
    #[inline]
    pub fn add<C: Component>(&mut self, entity: Id) -> EcsResult<()> {
        const_assert!(|C| size_of::<C>() == 0, "can't use add for non-ZST");

        match self.type_map.get::<C>() {
            Some(&id) => add_tag(self, entity, id),
            None => return Err(unregistered_type::<C>()),
        }
    }

    /// Checks if the entity has the component.
    pub fn has_id(&self, entity: Id, id: impl Into<Id>) -> EcsResult<bool> {
        has_component(self, entity, id.into())
    }

    /// Checks if entity has the component.
    pub fn has<C: Component>(&self, entity: Id) -> EcsResult<bool> {
        match self.type_map.get::<C>() {
            Some(&id) => has_component(self, entity, id),
            None => return Err(unregistered_type::<C>()),
        }
    }

    #[inline(always)]
    pub fn set_id<C>(&mut self, entity: Id, id: impl Into<Id>, val: C) -> EcsResult<Option<C>> {
        set_component(self, entity, id.into(), val)
    }

    #[inline(always)]
    pub fn set<C: Component>(&mut self, entity: Id, val: C) -> EcsResult<Option<C>> {
        match self.type_map.get::<C>() {
            Some(&id) => set_component(self, entity, id, val),
            None => return Err(unregistered_type::<C>()),
        }
    }

    #[inline(always)]
    pub fn get_id(&self, entity: Id, id: impl Into<Id>) -> EcsResult<Ptr> {
        get_component(self, entity, id.into())
    }

    #[inline(always)]
    pub fn get<C: Component>(&self, entity: Id) -> EcsResult<&C> {
        const_assert!(|C| size_of::<C>() != 0, "can't use get for ZST");

        match self.type_map.get::<C>() {
            // SAFETY: Type matches component id
            Some(&id) => get_component(self, entity, id).map(|ptr| unsafe { ptr.as_ref() }),
            None => return Err(unregistered_type::<C>()),
        }
    }

    #[inline(always)]
    pub fn get_id_mut(&mut self, entity: Id, id: impl Into<Id>) -> EcsResult<PtrMut> {
        get_component_mut(self, entity, id.into())
    }

    #[inline(always)]
    pub fn get_mut<C: Component>(&mut self, entity: Id) -> EcsResult<&mut C> {
        const_assert!(|C| size_of::<C>() != 0, "can't use get_mut for ZST");

        match self.type_map.get::<C>() {
            // SAFETY: Type matches component id
            Some(&id) => get_component_mut(self, entity, id).map(|ptr| unsafe { ptr.as_mut() }),
            None => return Err(unregistered_type::<C>()),
        }
    }

    #[inline(always)]
    pub fn is_alive(&self, entity: Id) -> bool {
        self.id_index.is_alive(entity)
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
