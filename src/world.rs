use crate::{
    component::{Component, ComponentDescriptor, ComponentInfo, private::Passkey},
    error::{EcsResult, GetError, MissingType},
    flags::{EntityFlags, TableFlags},
    get_params::Params,
    graph::GraphNode,
    id::{
        Entity, Id, IdMap, Signature,
        entity_manager::{EntityLocation, EntityManager, EntityRecord},
    },
    storage::table::{Table, TableData},
    table_index::{TableId, TableIndex},
    type_info::TypeMap,
    type_traits::{Data, TypedId},
    world_utils::{add_id, has_id, set_id},
};

pub struct World {
    pub(crate) entity_manager: EntityManager,
    pub(crate) type_arr: Vec<Option<Entity>>,
    pub(crate) type_map: TypeMap<Entity>,
    pub(crate) components: IdMap<ComponentInfo>,
    pub(crate) tables: TableIndex,
    pub(crate) root_table: TableId,
}

impl World {
    pub fn new() -> Self {
        let mut table_index = TableIndex::new();
        let root_table = table_index.add_with_id(|id| Table {
            id,
            _flags: TableFlags::empty(),
            signature: Signature::from(vec![]),
            data: TableData::new(Box::from([])),
            column_map: IdMap::new(256),
            node: GraphNode::new(),
        });

        Self {
            entity_manager: EntityManager::new(),
            type_arr: Vec::new(),
            type_map: TypeMap::new(),
            components: IdMap::new(256),
            tables: table_index,
            root_table,
        }
    }

    /// Gets the id for the type.
    #[inline(always)]
    pub fn id<T: TypedId>(&self) -> Result<T::Id, MissingType> {
        T::id(self)
    }

    /// Registers the type with the world if not registered and returns its id.
    ///
    /// This function eagerly evaluates `desc` (see [World::register_with]
    /// for lazily evaluated descriptor).
    pub fn register<T: Component>(&mut self, desc: T::DescType) -> Entity {
        let id = T::get_or_register_type(self);

        if !id.map_contains_key(&self.components) {
            desc.build(self, id, Passkey);
        }

        id
    }

    /// Registers the type with the world or returns its id if already registered.
    ///
    /// Lazily evaluates the descriptor and only calls it if the type is not registered.
    pub fn register_with<T: Component>(&mut self, f: impl Fn() -> T::DescType) -> Entity {
        let id = T::get_or_register_type(self);

        if !id.map_contains_key(&self.components) {
            f().build(self, id, Passkey);
        }

        id
    }

    /// Creates a component from this `id` if one doesn't exist.
    ///
    /// Returns `false` if:
    /// - `id` is already a component/tag.
    /// - `id` is not valid.
    #[inline(always)]
    pub fn to_component<T>(&mut self, id: Entity, f: impl FnOnce() -> T) -> bool
    where
        T: ComponentDescriptor,
    {
        if !self.is_alive(id) || id.map_contains_key(&self.components) {
            false
        } else {
            f().build(self, id, Passkey);
            true
        }
    }

    /// Creates a new component and returns its [Entity].
    ///
    /// Useful for creating "newtype" components.
    pub fn new_component<T>(&mut self, desc: T) -> Entity
    where
        T: ComponentDescriptor,
    {
        let id = self.new_entity();
        desc.build(self, id, Passkey);
        id
    }

    /// Creates a new [Entity].
    pub fn new_entity(&mut self) -> Entity {
        let root = self.root_table;

        self.entity_manager.new_id(|id| EntityRecord {
            location: EntityLocation {
                table: root,
                row: unsafe { self.tables[root].data.new_row(id) },
            },
            flags: EntityFlags::default(),
        })
    }

    /// Add `id` as tag to entity. No side effect if entity already has tag.
    #[inline]
    pub fn add_id(&mut self, e: Entity, id: impl Id) -> EcsResult<()> {
        add_id(self, e, id)
    }

    /// Add the type as tag to `id`. No side effect if `id` already has tag.
    #[inline]
    pub fn add<T: TypedId>(&mut self, e: Entity) -> EcsResult<()> {
        add_id(self, e, T::id(self)?)
    }

    /// Checks if the `id` has the component.
    pub fn has_id(&self, e: Entity, id: impl Id) -> bool {
        has_id(self, e, id)
    }

    /// Checks if `id` has the component.
    pub fn has<T: TypedId>(&self, e: Entity) -> bool {
        T::id(self).is_ok_and(|id| has_id(self, e, id))
    }

    #[inline]
    pub fn set<T: TypedId>(&mut self, e: Entity, val: T::Data) -> Option<T::Data>
    where
        T::Data: Component<DataType = Data>,
    {
        // SAFETY:
        // The component id is obtained from the type, so the data type matches.
        unsafe { set_id(self, e, T::id(self).ok()?, val) }
    }

    #[inline(always)]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entity_manager.is_alive(entity)
    }
}

const fn assert_immutable<T: Params>() {
    assert!(
        T::ALL_IMMUTABLE,
        "immutable World ref requires all Params to be immutable"
    )
}

pub trait WorldGet<'a> {
    fn get<T: Params>(self, id: Entity) -> Result<T::ParamsType<'a>, GetError>;
}

pub trait WorldMap<'a, Ret> {
    fn map<T: Params>(
        self,
        id: Entity,
        f: impl FnOnce(T::ParamsType<'a>) -> Ret,
    ) -> Result<Ret, GetError>;
}

impl<'a> WorldGet<'a> for &'a World {
    #[inline]
    fn get<T: Params>(self, id: Entity) -> Result<T::ParamsType<'a>, GetError> {
        const { assert_immutable::<T>() };
        T::create(self.into(), id)
    }
}

impl<'a, Ret> WorldMap<'a, Ret> for &'a World {
    #[inline]
    fn map<T: Params>(
        self,
        id: Entity,
        f: impl FnOnce(T::ParamsType<'a>) -> Ret,
    ) -> Result<Ret, GetError> {
        const { assert_immutable::<T>() };
        T::create(self.into(), id).map(f)
    }
}

impl<'a> WorldGet<'a> for &'a mut World {
    #[inline]
    fn get<T: Params>(self, id: Entity) -> Result<T::ParamsType<'a>, GetError> {
        T::create(self, id)
    }
}

impl<'a, Ret> WorldMap<'a, Ret> for &'a mut World {
    #[inline]
    fn map<T: Params>(
        self,
        id: Entity,
        f: impl FnOnce(T::ParamsType<'a>) -> Ret,
    ) -> Result<Ret, GetError> {
        T::create(self, id).map(f)
    }
}
