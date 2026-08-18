use std::sync::atomic::AtomicU32;

use crate::{
    ComponentKey, RelationKey,
    component::{
        self, ComponentConfig, ComponentRegisterError,
        id::{ComponentId, IntoComponentId},
        registry::ComponentRegistry,
        resolve,
    },
    error::{EcsError, EcsResult},
    id::{
        Id,
        allocator::{IdAllocator, IdRecord},
    },
    key::unregistered,
    relation::{
        RelationRegisterError, RelationRegistry,
        id::{IntoRelationId, RelationId},
    },
    table::{self},
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
    pub fn component_id<T: IntoComponentId>(&self, key: T) -> Option<ComponentId> {
        key.into_id(self)
    }

    #[inline(always)]
    pub fn relation_id<T: IntoRelationId>(&self, key: T) -> Option<RelationId> {
        key.into_id(self)
    }

    #[inline(always)]
    pub fn component<T: HasMeta>(&mut self, key: &ComponentKey<T>) -> Result<ComponentId, ComponentRegisterError> {
        self.components.register(key)
    }

    #[inline(always)]
    pub fn relation<T: HasMeta>(&mut self, key: &RelationKey<T>) -> Result<RelationId, RelationRegisterError> {
        self.relations.register(key)
    }

    /// Creates a new component and returns its [Id].
    ///
    /// Useful for creating "newtype" runtime components.
    #[inline(always)]
    pub fn new_component(&mut self, config: ComponentConfig) -> Result<ComponentId, ComponentRegisterError> {
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
    pub fn has<T: HasMeta>(&self, id: Id, key: &ComponentKey<T>) -> EcsResult<bool> {
        let r = self.ids.get(id)?;
        let c = resolve(self, key)?;
        Ok(component::has(self, r, c))
    }

    #[inline]
    pub fn insert<T: HasMeta>(&mut self, id: Id, key: &ComponentKey<T>, value: T) -> EcsResult<Option<T>> {
        let c = resolve(self, key)?;
        Ok(unsafe { component::insert(self, id, c, value)? })
    }

    #[inline]
    pub fn add<T: HasMeta + Default>(&mut self, id: Id, key: &ComponentKey<T>) -> EcsResult<Option<T>> {
        self.insert(id, key, T::default())
    }

    #[inline]
    pub fn remove<T: HasMeta>(&mut self, id: Id, key: &ComponentKey<T>) -> EcsResult<()> {
        let c = resolve(self, key)?;
        unsafe { component::remove(self, id, c)? };
        Ok(())
    }

    #[inline]
    pub fn get<T: HasMeta>(&self, id: Id, key: &ComponentKey<T>) -> EcsResult<&T> {
        let r = self.ids.get(id)?;
        let c = resolve(self, key)?;
        unsafe { component::get(self, r, c) }.ok_or(EcsError::MissingComponent(id, c))
    }

    #[inline]
    pub fn get_mut<T: HasMeta>(&mut self, id: Id, key: &ComponentKey<T>) -> EcsResult<&mut T> {
        let r = self.ids.get(id)?;
        let c = self.components.find(key).ok_or_else(|| unregistered(key.untyped()))?;
        unsafe { component::get_mut(self, r, c) }.ok_or(EcsError::MissingComponent(id, c))
    }

    #[inline]
    pub fn relate<T: HasMeta>(&mut self, id: Id, key: &RelationKey<T>, target: Id, payload: T) -> EcsResult<()> {
        let r = self.relations.find(key).ok_or_else(|| unregistered(key.untyped()))?;
        unsafe { self.relations[r].relate(id, target, payload)? };
        Ok(())
    }

    #[inline]
    pub fn unrelate<T: HasMeta>(&mut self, id: Id, key: &RelationKey<T>, target: Id) -> EcsResult<()> {
        let r = self.relations.find(key).ok_or_else(|| unregistered(key.untyped()))?;
        self.relations[r].unrelate(id, target);
        Ok(())
    }

    #[inline]
    pub fn remove_relation<T: HasMeta>(&mut self, id: Id, key: &RelationKey<T>) -> EcsResult<()> {
        let r = self.relations.find(key).ok_or_else(|| unregistered(key.untyped()))?;
        self.relations[r].purge(id);
        Ok(())
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

impl Default for Ecs {
    fn default() -> Self {
        Self::new()
    }
}
