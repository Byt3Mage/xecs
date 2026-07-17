use crate::{
    component::{self, ComponentConfig, ComponentId, StaticId, TypedStaticId, registry::ComponentRegistry},
    error::{EcsResult, Error},
    id::{
        Id,
        allocator::{IdAllocator, IdRecord, NotAlive},
    },
    relation::RelationRegistry,
    storage::table::{self},
    table_index::TableIndex,
};

pub struct Ecs {
    pub(crate) ids: IdAllocator,
    pub(crate) components: ComponentRegistry,
    pub(crate) relations: RelationRegistry,
    pub(crate) tables: TableIndex,
    pub(crate) generation: u64,
    pub(crate) static_ids: Vec<Option<ComponentId>>,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            ids: IdAllocator::new(),
            components: ComponentRegistry::new(),
            relations: RelationRegistry::new(),
            tables: TableIndex::new(),
            generation: 0,
            static_ids: Vec::new(),
        }
    }

    /// Gets the [ComponentId] for the static id.
    #[inline(always)]
    pub fn id<T>(&self, comp: &StaticId<T>) -> EcsResult<ComponentId> {
        match self.static_ids.get(comp.id() as usize) {
            Some(Some(id)) => Ok(*id),
            Some(None) | None => Err(Error::UnregisteredStatic(comp.id())),
        }
    }

    #[inline(always)]
    pub fn id_t<T: TypedStaticId>(&self) -> EcsResult<ComponentId> {
        self.id(T::id())
    }

    /// Registers the type with the ecs or returns its [ComponentId] if already registered.
    ///
    /// Lazily evaluates the descriptor and only calls it if the type is not registered.
    pub fn register_with<T, F>(&mut self, comp: &StaticId<T>, f: F) -> ComponentId
    where
        F: FnOnce() -> ComponentConfig,
    {
        let idx = comp.id() as usize;

        if idx >= self.static_ids.len() {
            self.static_ids.resize(idx + 1, None);
        }

        *self.static_ids[idx].get_or_insert_with(|| self.components.register(f()))
    }

    /// Registers the component with the ecs if not registered and returns its [Id].
    ///
    /// This function uses the default builder value (see [ECS::register_with]
    /// for lazily evaluated descriptor function).
    pub fn register<T>(&mut self, component: &StaticId<T>) -> ComponentId {
        self.register_with(component, || ComponentConfig::new().name(component.name()))
    }

    /// Registers the type with the ecs if not registered and returns its [Id].
    ///
    /// This function uses the default builder value (see [ECS::register_with]
    /// for lazily evaluated descriptor function).
    pub fn register_t<T: TypedStaticId>(&mut self) -> ComponentId {
        self.register(T::id())
    }

    /// Creates a new component and returns its [Id].
    ///
    /// Useful for creating "newtype" runtime components.
    #[inline]
    pub fn new_component(&mut self, builder: ComponentConfig) -> ComponentId {
        self.components.register(builder)
    }

    /// Creates a new [Id].
    pub fn new_id(&mut self) -> Id {
        // SAFETY: No columns in root table, so there's nothing to initialize.
        self.ids.new_id(|id| IdRecord {
            table: self.tables.root_id(),
            row: unsafe { self.tables.root_mut().data.alloc_row(id) },
        })
    }

    pub fn delete_id(&mut self, id: Id) -> EcsResult<()> {
        let r = self.ids.get(id)?;
        unsafe { table::remove_id(self, r.table, r.row) };
        self.ids.remove_id(id);
        Ok(())
    }

    /// Checks if the `id` has the component.
    #[inline(always)]
    pub fn has_id(&self, id: Id, component: ComponentId) -> Result<bool, NotAlive> {
        self.ids.get(id).map(|r| component::has(self, r, component))
    }

    /// Checks if `id` has the static component `T`.
    #[inline(always)]
    pub fn has<T>(&self, id: Id, component: &StaticId<T>) -> EcsResult<bool> {
        self.has_id(id, self.id(component)?).map_err(Error::NotAlive)
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
    pub fn remove_id(&mut self, id: Id, component: ComponentId) -> Result<(), NotAlive> {
        unsafe { component::remove(self, id, component) }
    }

    #[inline]
    pub fn remove<T>(&mut self, id: Id, component: &StaticId<T>) -> EcsResult<()> {
        self.remove_id(id, self.id(component)?).map_err(Error::NotAlive)
    }

    #[inline]
    pub fn remove_t<T: TypedStaticId>(&mut self, id: Id) -> EcsResult<()> {
        self.remove(id, T::id())
    }

    #[inline]
    pub fn get<T>(&self, id: Id, component: &StaticId<T>) -> EcsResult<&T> {
        let r = self.ids.get(id)?;
        let component = self.id(component)?;
        unsafe { component::get(self, r, component) }.ok_or(Error::MissingComponent { id, component })
    }

    #[inline]
    pub fn get_mut<T>(&mut self, id: Id, component: &StaticId<T>) -> EcsResult<&mut T> {
        let r = self.ids.get(id)?;
        let component = self.id(component)?;
        unsafe { component::get_mut(self, r, component) }.ok_or(Error::MissingComponent { id, component })
    }

    #[inline]
    pub fn get_t<T: TypedStaticId>(&self, id: Id) -> EcsResult<&T> {
        self.get(id, T::id())
    }

    #[inline]
    pub fn get_mut_t<T: TypedStaticId>(&mut self, id: Id) -> EcsResult<&mut T> {
        self.get_mut(id, T::id())
    }

    #[inline(always)]
    pub fn is_alive(&self, id: Id) -> bool {
        self.ids.is_alive(id)
    }

    #[inline(always)]
    pub fn alive_count(&self) -> usize {
        self.ids.num_alive()
    }

    #[inline(always)]
    pub fn dead_count(&self) -> usize {
        self.ids.num_dead()
    }
}

// /// Wrapper trait to make the API nicer to write
// pub trait QuerySingle<R> {
//     fn query_id<T: GetMulti>(&mut self, id: Id, query: T::Query, f: impl FnOnce(T::Output<'_>) -> R) -> EcsResult<R>;
// }

// impl<R> QuerySingle<R> for Ecs {
//     // TODO: get table once
//     fn query_id<T: GetMulti>(&mut self, id: Id, query: T::Query, f: impl FnOnce(T::Output<'_>) -> R) -> EcsResult<R> {
//         T::get(self, id, query).map(f)
//     }
// }
