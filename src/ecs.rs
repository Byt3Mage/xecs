use crate::{
    component::{self, ComponentBuilder, ComponentMeta, Params, StaticId, TypedStaticId},
    error::{EcsResult, Error, InvalidId},
    graph::GraphNode,
    id::{
        Id, Signature,
        manager::{IdManager, IdRecord},
        map::IdMap,
    },
    storage::table::{Table, TableData},
    table_index::{TableId, TableIndex},
    unsafe_ecs::UnsafeEcsCell,
};

pub struct Ecs {
    pub(crate) ids: IdManager,
    pub(crate) components: IdMap<ComponentMeta>,
    pub(crate) component_ids: Vec<Option<Id>>,
    pub(crate) tables: TableIndex,
    pub(crate) root_table: TableId,
}

impl Ecs {
    pub fn new() -> Self {
        let mut tables = TableIndex::new();

        let root_table = tables.add(Table {
            sig: Signature::from(vec![]),
            data: TableData::new(Box::new([])),
            col_map: IdMap::new(),
            graph_node: GraphNode::new(),
        });

        Self {
            ids: IdManager::new(),
            components: IdMap::new(),
            component_ids: Vec::new(),
            tables,
            root_table,
        }
    }

    fn get_or_create_id<T>(&mut self, comp: &StaticId<T>) -> Id {
        let idx = comp.id() as usize;

        if idx >= self.component_ids.len() {
            self.component_ids.resize(idx + 1, None);
        }

        if let Some(id) = self.component_ids[idx] {
            return id;
        }

        let new_id = self.new_id();
        self.component_ids[idx] = Some(new_id);

        new_id
    }

    /// Gets the [Id] for the component.
    #[inline(always)]
    pub fn id<T>(&self, comp: &StaticId<T>) -> EcsResult<Id> {
        match self.component_ids.get(comp.id() as usize) {
            Some(Some(id)) => Ok(*id),
            _ => Err(Error::UnregisteredComponent(comp.name())),
        }
    }

    #[inline(always)]
    pub fn id_t<T: TypedStaticId>(&self) -> EcsResult<Id> {
        self.id(T::id())
    }

    /// Registers the type with the ecs or returns its [Id] if already registered.
    ///
    /// Lazily evaluates the descriptor and only calls it if the type is not registered.
    pub fn register_with<T, F>(&mut self, comp: &StaticId<T>, f: F) -> Id
    where
        F: FnOnce() -> ComponentBuilder<T>,
    {
        let id = self.get_or_create_id(comp);
        if !self.components.contains(id) {
            f().build(&mut self.components, id);
        }
        id
    }

    /// Registers the component with the ecs if not registered and returns its [Id].
    ///
    /// This function uses the default builder value (see [ECS::register_with]
    /// for lazily evaluated descriptor function).
    pub fn register<T>(&mut self, component: &StaticId<T>) -> Id {
        let builder = ComponentBuilder::new()
            .name(component.name())
            .storage(component.storage());
        self.register_with(component, || builder)
    }

    /// Registers the type with the ecs if not registered and returns its [Id].
    ///
    /// This function uses the default builder value (see [ECS::register_with]
    /// for lazily evaluated descriptor function).
    pub fn register_t<T: TypedStaticId>(&mut self) -> Id {
        self.register(T::id())
    }

    /// Creates a component from this `id` if one doesn't exist.
    ///
    /// Returns `false` if:
    /// - `id` is already a component
    /// - `id` is not valid
    #[inline(always)]
    pub fn to_component<T, F>(&mut self, id: Id, f: F) -> bool
    where
        T: 'static,
        F: FnOnce() -> ComponentBuilder<T>,
    {
        if !self.is_alive(id) || self.components.contains(id) {
            return false;
        }
        f().build(&mut self.components, id);
        true
    }

    /// Creates a new component and returns its [Id].
    ///
    /// Useful for creating "newtype" runtime components.
    pub fn new_component<T>(&mut self, builder: ComponentBuilder<T>) -> Id {
        let id = self.new_id();
        builder.build(&mut self.components, id);
        id
    }

    /// Creates a new [Id].
    pub fn new_id(&mut self) -> Id {
        // SAFETY: Root table does not have columns,
        // so it's safe leave uninitialized.
        self.ids.new_id(|id| IdRecord {
            table: self.root_table,
            row: unsafe { self.tables[self.root_table].data.alloc_row(id) },
        })
    }

    /// Checks if the `id` has the component.
    #[inline(always)]
    pub fn has_id(&self, id: Id, component: Id) -> EcsResult<bool> {
        component::has(self, id, component)
    }

    /// Checks if `id` has the typed component `T`.
    #[inline(always)]
    pub fn has<T>(&self, id: Id, component: &StaticId<T>) -> EcsResult<bool> {
        self.has_id(id, self.id(component)?)
    }

    /// Checks if `id` has the typed component `T`.
    #[inline(always)]
    pub fn has_t<T: TypedStaticId>(&self, id: Id) -> EcsResult<bool> {
        self.has(id, T::id())
    }

    #[inline]
    pub fn insert<T>(&mut self, id: Id, component: &StaticId<T>, value: T) -> EcsResult<Option<T>> {
        unsafe { component::insert(self, id, self.id(component)?, value) }
    }

    #[inline]
    pub fn insert_t<T: TypedStaticId>(&mut self, id: Id, value: T) -> EcsResult<Option<T>> {
        self.insert(id, T::id(), value)
    }

    #[inline]
    pub fn remove_id(&mut self, id: Id, component: Id) -> EcsResult<()> {
        unsafe { component::remove(self, id, component) }
    }

    #[inline]
    pub fn remove<T>(&mut self, id: Id, component: &StaticId<T>) -> EcsResult<()> {
        self.remove_id(id, self.id(component)?)
    }

    #[inline]
    pub fn remove_t<T: TypedStaticId>(&mut self, id: Id) -> EcsResult<()> {
        self.remove(id, T::id())
    }

    #[inline]
    pub fn get<T>(&self, id: Id, component: &StaticId<T>) -> EcsResult<&T> {
        let r = self.ids.get(id).ok_or(InvalidId(id))?;
        unsafe { component::get(self, id, r, self.id(component)?) }
    }

    #[inline]
    pub fn get_mut<T>(&mut self, id: Id, component: &StaticId<T>) -> EcsResult<&mut T> {
        let r = self.ids.get(id).ok_or(InvalidId(id))?;
        unsafe { component::get_mut(self, id, r, self.id(component)?) }
    }

    #[inline]
    pub fn get_t<T: TypedStaticId>(&self, id: Id) -> EcsResult<&T> {
        self.get(id, T::id())
    }

    #[inline]
    pub fn get_mut_t<T: TypedStaticId>(&mut self, id: Id) -> EcsResult<&mut T> {
        self.get_mut(id, T::id())
    }

    pub fn get_multi<T: Params>(&self, id: Id) -> EcsResult<T::ParamsType<'_>> {
        const {
            assert!(T::ALL_IMMUTABLE, "use get_many_mut for mutable access");
        }

        unsafe { T::create(UnsafeEcsCell::new(self), id) }
    }

    pub fn get_multi_mut<T: Params>(&mut self, id: Id) -> EcsResult<T::ParamsType<'_>> {
        const { panic!("Validate no conflicting mutable access") }
        unsafe { T::create(UnsafeEcsCell::new_mut(self), id) }
    }

    #[inline]
    pub fn insert_singleton<T>(&mut self, component: &StaticId<T>, val: T) -> EcsResult<Option<T>> {
        component::insert_singleton(self, self.id(component)?, val)
    }

    #[inline]
    pub fn insert_singleton_t<T: TypedStaticId>(&mut self, val: T) -> EcsResult<Option<T>> {
        self.insert_singleton(T::id(), val)
    }

    #[inline(always)]
    pub fn is_alive(&self, id: Id) -> bool {
        self.ids.is_alive(id)
    }

    #[inline(always)]
    pub fn alive_count(&self) -> usize {
        self.ids.num_alive()
    }
}
