use crate::{
    component::{ComponentDescriptor, ComponentInfo, private::Passkey},
    error::{EcsResult, GetResult, UnregisteredTypeErr},
    flags::{IdFlags, TableFlags},
    get_params::Params,
    graph::GraphNode,
    id::{
        Id, IdMap, IntoId, Signature,
        manager::{IdLocation, IdManager, IdRecord},
    },
    registration::ComponentId,
    storage::table::{self, Table},
    table_index::{TableId, TableIndex},
    type_info::TypeMap,
    type_traits::{DataComponent, TagComponent, TypedId},
    world_utils::{add_tag, has_component, set_component, set_component_checked},
};

pub struct World {
    pub(crate) id_manager: IdManager,
    pub(crate) type_arr: Vec<Option<Id>>,
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
            signature: Signature::from(vec![]),
            id_data: table::ComponentData::new(Box::from([])),
            column_map: IdMap::new(),
            node: GraphNode::new(),
        });

        Self {
            id_manager: IdManager::new(),
            type_arr: Vec::new(),
            type_map: TypeMap::new(),
            components: IdMap::new(),
            table_index,
            root_table,
        }
    }

    /// Gets the entity id for the type.
    /// Returns `None` if type is not registered with this world.
    #[inline(always)]
    pub fn id<T: TypedId>(&self) -> Result<Id, UnregisteredTypeErr> {
        T::id(self)
    }

    /// Registers the type with the world if not registered and returns its id.
    ///
    /// This function eagerly evaluates `desc` (see [World::register_with]
    /// for lazily evaluated descriptor).
    pub fn register<T: ComponentId>(&mut self, desc: T::DescType) -> Id {
        let id = T::get_or_register_type(self);

        if !self.components.contains(id) {
            desc.build(self, id, Passkey);
        }

        id
    }

    /// Registers the type with the world or returns its id if already registered.
    ///
    /// Lazily evaluates the descriptor and only calls it if the type is not registered.
    pub fn register_with<T>(&mut self, f: impl Fn() -> T::DescType) -> Id
    where
        T: ComponentId,
    {
        let id = T::get_or_register_type(self);

        if !self.components.contains(id) {
            f().build(self, id, Passkey);
        }

        id
    }

    /// Creates a component from this `id` if one doesn't exist.
    ///
    /// Returns `false` if:
    /// - `id` is already a component/tag.
    /// - `id` is a pair.
    /// - `id` is not valid.
    #[inline(always)]
    pub fn to_component<T>(&mut self, id: Id, f: impl FnOnce() -> T) -> bool
    where
        T: ComponentDescriptor,
    {
        if id.is_pair() || !self.is_alive(id) || self.components.contains(id) {
            false
        } else {
            f().build(self, id, Passkey);
            true
        }
    }

    /// Creates a new component and returns its [Id].
    ///
    /// Useful for creating "newtype" components.
    pub fn new_component<T>(&mut self, desc: T) -> Id
    where
        T: ComponentDescriptor,
    {
        let id = self.new_id();
        desc.build(self, id, Passkey);
        id
    }

    /// Creates a new [Id].
    pub fn new_id(&mut self) -> Id {
        let root = self.root_table;
        self.id_manager.new_id(|id| IdRecord {
            location: IdLocation {
                table: root,
                row: unsafe { self.table_index[root].id_data.new_row(id) },
            },
            flags: IdFlags::default(),
        })
    }

    /// Add `comp` as tag to `id`. No side effect if `id` already has tag.
    #[inline]
    pub fn add_id(&mut self, id: Id, comp: impl IntoId) -> EcsResult<()> {
        debug_assert!(comp.validate(self), "id or pair is not valid");
        add_tag(self, id, comp.into_id())
    }

    /// Add the type as tag to `id`. No side effect if `id` already has tag.
    #[inline]
    pub fn add<T: TypedId + TagComponent>(&mut self, id: Id) -> EcsResult<()> {
        add_tag(self, id, T::id(self)?)
    }

    /// Checks if the `id` has the component.
    pub fn has_id(&self, id: Id, comp: impl IntoId) -> bool {
        debug_assert!(comp.validate(self), "id or pair is not valid");
        has_component(self, id, comp.into_id())
    }

    /// Checks if `id` has the component.
    pub fn has<T: TypedId>(&self, id: Id) -> bool {
        T::id(self).is_ok_and(|comp| has_component(self, id, comp))
    }

    #[inline(always)]
    pub fn set_id<T>(&mut self, id: Id, comp: impl IntoId, val: T) -> Option<T>
    where
        T: DataComponent,
    {
        debug_assert!(comp.validate(self), "id or pair is not valid");
        set_component_checked(self, id, comp.into_id(), val)
    }

    #[inline]
    pub fn set<T: TypedId>(&mut self, id: Id, val: T::Data) -> Option<T::Data>
    where
        T::Data: DataComponent,
    {
        // SAFETY:
        // The component id is obtained from the type, so the data type matches.
        unsafe { set_component(self, id, T::id(self).ok()?, val) }
    }

    #[inline(always)]
    pub fn is_alive(&self, entity: Id) -> bool {
        self.id_manager.is_alive(entity)
    }
}

const fn assert_immutable<T: Params>() {
    assert!(
        T::ALL_IMMUTABLE,
        "immutable World ref requires all Params to be immutable"
    )
}

pub trait WorldGet<'a> {
    fn get<T: Params>(self, id: Id) -> GetResult<T::ParamsType<'a>>;
}

pub trait WorldMap<'a, Ret> {
    fn map<T: Params>(self, id: Id, f: impl FnOnce(T::ParamsType<'a>) -> Ret) -> GetResult<Ret>;
}

impl<'a> WorldGet<'a> for &'a World {
    #[inline]
    fn get<T: Params>(self, id: Id) -> GetResult<T::ParamsType<'a>> {
        const { assert_immutable::<T>() };
        T::create(self.into(), id)
    }
}

impl<'a, Ret> WorldMap<'a, Ret> for &'a World {
    #[inline]
    fn map<T: Params>(self, id: Id, f: impl FnOnce(T::ParamsType<'a>) -> Ret) -> GetResult<Ret> {
        const { assert_immutable::<T>() };
        T::create(self.into(), id).map(f)
    }
}

impl<'a> WorldGet<'a> for &'a mut World {
    #[inline]
    fn get<T: Params>(self, id: Id) -> GetResult<T::ParamsType<'a>> {
        T::create(self, id)
    }
}

impl<'a, Ret> WorldMap<'a, Ret> for &'a mut World {
    #[inline]
    fn map<T: Params>(self, id: Id, f: impl FnOnce(T::ParamsType<'a>) -> Ret) -> GetResult<Ret> {
        T::create(self, id).map(f)
    }
}
