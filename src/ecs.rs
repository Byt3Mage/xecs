use std::sync::atomic::AtomicU32;

use crate::{
    component::{
        self, ComponentConfig, ComponentInfo,
        id::{ComponentId, StaticId, TypedStaticId},
        registry::{ComponentRegistry, Unregistered},
    },
    error::{EcsError, EcsResult},
    id::{
        Id,
        allocator::{IdAllocator, IdRecord},
    },
    relation::{RelationId, RelationRegistry},
    storage::table::{self},
    table_index::TableIndex,
    type_meta::HasMeta,
};

fn ecs_id_allocate() -> u32 {
    static MAX_ID: AtomicU32 = AtomicU32::new(0);
    MAX_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub struct Ecs {
    pub(crate) ids: IdAllocator,
    pub(crate) components: ComponentRegistry,
    pub(crate) relations: RelationRegistry,
    pub(crate) tables: TableIndex,
    pub(crate) generation: u32,
    unique_id: u32,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            ids: IdAllocator::new(),
            components: ComponentRegistry::new(),
            relations: RelationRegistry::new(),
            tables: TableIndex::new(),
            generation: 0,
            unique_id: ecs_id_allocate(),
        }
    }

    #[inline(always)]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    #[inline(always)]
    pub fn unique_id(&self) -> u32 {
        self.unique_id
    }

    #[inline(always)]
    pub fn component_id<T: HasMeta>(&self, id: &'static StaticId<T>) -> Result<ComponentId, Unregistered> {
        self.components.find(id)
    }

    pub fn component<T: HasMeta>(&self, id: &'static StaticId<T>) -> Result<&ComponentInfo, Unregistered> {
        self.components.find(id).map(|id| self.components.get(id))
    }

    #[inline(always)]
    pub fn component_id_t<T: TypedStaticId>(&self) -> Result<ComponentId, Unregistered> {
        self.component_id(T::id())
    }

    #[inline(always)]
    pub fn register<T: HasMeta>(&mut self, component: &StaticId<T>) -> ComponentId {
        self.components.register(component)
    }

    /// Creates a new component and returns its [Id].
    ///
    /// Useful for creating "newtype" runtime components.
    #[inline(always)]
    pub fn new_component(&mut self, config: ComponentConfig) -> ComponentId {
        self.components.new_component(config)
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

    /// Checks if `id` has the `component`.
    #[inline(always)]
    pub fn has<T: HasMeta>(&self, id: Id, component: &'static StaticId<T>) -> EcsResult<bool> {
        let r = self.ids.get(id)?;
        let component = self.components.find(component).unwrap();
        Ok(component::has(self, r, component))
    }

    /// Checks if `id` has the typed component `T`.
    #[inline(always)]
    pub fn has_t<T: TypedStaticId>(&self, id: Id) -> EcsResult<bool> {
        self.has(id, T::id())
    }

    #[inline]
    pub fn insert<T: HasMeta>(&mut self, id: Id, component: &'static StaticId<T>, value: T) -> EcsResult<Option<T>> {
        let component = self.components.find(component)?;
        Ok(unsafe { component::insert(self, id, component, value)? })
    }

    #[inline]
    pub fn insert_t<T: TypedStaticId>(&mut self, id: Id, value: T) -> EcsResult<Option<T>> {
        self.insert(id, T::id(), value)
    }

    #[inline]
    pub fn remove<T: HasMeta>(&mut self, id: Id, component: &'static StaticId<T>) -> EcsResult<()> {
        let component = self.components.find(component)?;
        Ok(unsafe { component::remove(self, id, component)? })
    }

    #[inline]
    pub fn remove_t<T: TypedStaticId>(&mut self, id: Id) -> EcsResult<()> {
        self.remove(id, T::id())
    }

    #[inline]
    pub fn get<T: HasMeta>(&self, id: Id, component: &'static StaticId<T>) -> EcsResult<&T> {
        let r = self.ids.get(id)?;
        let component = self.components.find(component)?;
        unsafe { component::get(self, r, component) }.ok_or(EcsError::MissingComponent { id, component })
    }

    #[inline]
    pub fn get_mut<T: HasMeta>(&mut self, id: Id, component: &'static StaticId<T>) -> EcsResult<&mut T> {
        let r = self.ids.get(id)?;
        let component = self.components.find(component)?;
        unsafe { component::get_mut(self, r, component) }.ok_or(EcsError::MissingComponent { id, component })
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
    pub fn relate(&mut self, id: Id, relation: RelationId, target: Id) {
        self.relations.index_mut(relation).relate(id, target);
    }

    #[inline]
    pub fn unrelate(&mut self, id: Id, relation: RelationId, target: Id) {
        self.relations.index_mut(relation).unrelate(id, target);
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
