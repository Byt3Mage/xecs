use crate::{
    component::{Component, ComponentDesc, ComponentInfo, TagDesc},
    error::{EcsResult, unregistered_type},
    flags::{IdFlags, TableFlags},
    graph::GraphNode,
    id::{
        Id, IdList, IdMap, ToComponentId,
        id_index::{IdIndex, IdRecord},
    },
    query::Params,
    storage::{table::Table, table_data::TableData},
    table_index::{TableId, TableIndex},
    type_info::TypeMap,
    world_utils::{add_tag, has_component, set_component, set_component_checked},
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

    /// Creates a new [Id] for asso
    pub fn new_id(&mut self) -> Id {
        let root = self.root_table;
        self.id_index.new_id(|id| IdRecord {
            table: root,
            row: unsafe { self.table_index[root].data.new_row(id) },
            flags: IdFlags::default(),
        })
    }

    /// Add `id` as tag to entity. No side effect if entity already has tag.
    #[inline]
    pub fn add_id(&mut self, id: Id, comp: impl ToComponentId) {
        add_tag(self, id, comp.get_id(self).unwrap()).unwrap()
    }

    /// Add the type as tag to id. No side effect if id already has tag.
    #[inline]
    pub fn add<C: Component>(&mut self, id: Id) -> EcsResult<()> {
        const_assert!(|C| size_of::<C>() == 0, "can't use add for non-ZST");

        match self.type_map.get::<C>() {
            Some(&comp) => add_tag(self, id, comp),
            None => return Err(unregistered_type::<C>()),
        }
    }

    /// Checks if the id has the component.
    pub fn has_id(&self, id: Id, comp: impl ToComponentId) -> bool {
        comp.get_id(self)
            .map_or(false, |comp| has_component(self, id, comp))
    }

    /// Checks if id has the component.
    pub fn has<C: Component>(&self, id: Id) -> bool {
        self.type_map
            .get::<C>()
            .map_or(false, |&comp| has_component(self, id, comp))
    }

    /// # Safety
    /// - Caller must ensure that the `val` is a pointee to the same data type as
    #[inline(always)]
    pub unsafe fn set_id<C: Component>(
        &mut self,
        id: Id,
        comp: impl ToComponentId,
        val: C,
    ) -> Option<C> {
        set_component_checked(self, id, comp.get_id(self)?, val)
    }

    #[inline]
    pub fn set<C: Component>(&mut self, id: Id, val: C) -> Option<C> {
        unsafe { set_component(self, id, *self.type_map.get::<C>()?, val) }
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

pub trait WorldGet<Ret> {
    fn get<T: Params>(
        &mut self,
        id: Id,
        callback: impl for<'a> FnOnce(T::ParamType<'a>) -> Ret,
    ) -> EcsResult<Ret>;
}

impl<Ret> WorldGet<Ret> for World {
    #[inline]
    fn get<T: Params>(
        &mut self,
        id: Id,
        f: impl for<'a> FnOnce(T::ParamType<'a>) -> Ret,
    ) -> EcsResult<Ret> {
        Ok(f(T::create(self, id)?))
    }
}
