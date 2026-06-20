use crate::{
    Query,
    component::{self, Component, ComponentBuilder, StaticId, TypedStaticId},
    error::{EcsResult, Error},
    id::{
        Id,
        allocator::{IdAllocator, IdRecord, NotAlive},
        map::IdMap,
    },
    query::{
        iter::Columns,
        typed_query::{TQuery, TQueryBuilder},
    },
    storage::{
        Storage,
        table::{self},
    },
    table_index::TableIndex,
};

pub struct Ecs {
    pub(crate) ids: IdAllocator,
    pub(crate) components: IdMap<Component>,
    pub(crate) component_ids: Vec<Option<Id>>,
    pub(crate) tables: TableIndex,
    pub(crate) cached_queries: Vec<Query>,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            ids: IdAllocator::new(),
            components: IdMap::new(),
            component_ids: Vec::new(),
            cached_queries: Vec::new(),
            tables: TableIndex::new(),
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
            Some(Some(id)) => {
                if !self.ids.is_alive(*id) {
                    return Err(Error::NotAlive(NotAlive(*id)));
                }
                Ok(*id)
            }
            Some(None) | None => Err(Error::UnregisteredComponent(comp.name())),
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
        // SAFETY: No columns in root table, so there's nothing to initialize.
        self.ids.new_id(|id| IdRecord {
            table: self.tables.root_id(),
            row: unsafe { self.tables.root_mut().data.alloc_row(id) },
        })
    }

    pub fn delete_id(&mut self, id: Id) -> EcsResult<()> {
        let r = self.ids.get(id)?;

        unsafe { table::remove_id(self, r.table, r.row) };

        // TODO: consider optimization by caching
        // the sparse sets containing the id.
        for comp in self.components.values_mut() {
            if let Storage::Sparse(s) = &mut comp.storage {
                s.remove(id);
            }
        }

        self.components.remove(id);
        self.ids.remove_id(id);

        Ok(())
    }

    /// Checks if the `id` has the component.
    #[inline(always)]
    pub fn has_id(&self, id: Id, component: Id) -> EcsResult<bool> {
        component::has(self, id, component)
    }

    /// Checks if `id` has the static component `T`.
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
        let r = self.ids.get(id)?;
        let comp = self.id(component)?;
        unsafe { component::get(self, id, r, comp)?.ok_or(Error::MissingComponent { id, comp }) }
    }

    #[inline]
    pub fn get_mut<T>(&mut self, id: Id, component: &StaticId<T>) -> EcsResult<&mut T> {
        let r = self.ids.get(id)?;
        let comp = self.id(component)?;
        unsafe { component::get_mut(self, id, r, comp)?.ok_or(Error::MissingComponent { id, comp }) }
    }

    #[inline]
    pub fn get_t<T: TypedStaticId>(&self, id: Id) -> EcsResult<&T> {
        self.get(id, T::id())
    }

    #[inline]
    pub fn get_mut_t<T: TypedStaticId>(&mut self, id: Id) -> EcsResult<&mut T> {
        self.get_mut(id, T::id())
    }

    #[inline]
    pub fn insert_resource<T>(&mut self, component: &StaticId<T>, val: T) -> EcsResult<Option<T>> {
        unsafe { component::insert_resource(self, self.id(component)?, val) }
    }

    #[inline]
    pub fn insert_resource_t<T: TypedStaticId>(&mut self, val: T) -> EcsResult<Option<T>> {
        self.insert_resource(T::id(), val)
    }

    #[inline]
    pub fn resource<T>(&self, component: &StaticId<T>) -> EcsResult<&T> {
        let id = self.id(component)?;
        unsafe { component::resource(self, id)?.ok_or(Error::MissingResource { id }) }
    }

    #[inline]
    pub fn resource_t<T: TypedStaticId>(&self) -> EcsResult<&T> {
        self.resource(T::id())
    }

    #[inline]
    pub fn resource_mut<T>(&mut self, component: &StaticId<T>) -> EcsResult<&mut T> {
        let id = self.id(component)?;
        unsafe { component::resource_mut(self, id)?.ok_or(Error::MissingResource { id }) }
    }

    #[inline]
    pub fn resource_mut_t<T: TypedStaticId>(&mut self) -> EcsResult<&mut T> {
        self.resource_mut(T::id())
    }

    #[inline(always)]
    pub fn query_builder_t<'w, T: Columns>(&'w self) -> EcsResult<TQueryBuilder<'w, T>> {
        TQueryBuilder::new(self)
    }

    #[inline(always)]
    pub fn query_t<'w, T: Columns>(&'w self) -> EcsResult<TQuery<T>> {
        self.query_builder_t().map(TQueryBuilder::build)
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
