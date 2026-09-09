use std::sync::atomic::AtomicU32;

use crate::{
    ComponentKey, RelationKey,
    bundle::{self, BundleRegistry},
    component::{self, ComponentConfig, ComponentRegisterError, id::ComponentId, registry::ComponentRegistry},
    error::{EcsError, EcsResult},
    id::{
        Id,
        allocator::{IdAllocator, IdRecord},
    },
    relation::{self, RelationRegisterError, RelationRegistry, id::RelationId},
    table::{self},
    table_registry::TableRegistry,
    type_meta::HasMeta,
};

fn ecs_id_allocate() -> u32 {
    static MAX_ID: AtomicU32 = AtomicU32::new(0);
    MAX_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub struct Ecs {
    ecs_id: u32,
    pub(crate) ids: IdAllocator,
    pub(crate) components: ComponentRegistry,
    pub(crate) relations: RelationRegistry,
    pub(crate) bundles: BundleRegistry,
    pub(crate) tables: TableRegistry,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            ids: IdAllocator::new(),
            components: ComponentRegistry::new(),
            relations: RelationRegistry::new(),
            bundles: BundleRegistry::new(),
            tables: TableRegistry::new(),
            ecs_id: ecs_id_allocate(),
        }
    }

    #[inline(always)]
    pub fn generation(&self) -> u32 {
        self.tables.generation()
    }

    #[inline(always)]
    pub fn ecs_id(&self) -> u32 {
        self.ecs_id
    }

    #[inline(always)]
    pub fn component<T: HasMeta>(&mut self, key: ComponentKey<T>) -> Result<ComponentId, ComponentRegisterError> {
        self.components.register(key)
    }

    #[inline(always)]
    pub fn relation<T: HasMeta>(&mut self, key: &RelationKey) -> Result<RelationId, RelationRegisterError> {
        self.relations.register::<T>(key)
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
        let table = TableRegistry::ROOT;
        // SAFETY: No columns in root table, so there's nothing to initialize.
        self.ids.new_id(|id| IdRecord {
            table,
            row: unsafe { self.tables[table].data.alloc_row(id) },
        })
    }

    pub fn delete_id(&mut self, id: Id) -> EcsResult<()> {
        // TODO: run remove hooks.
        let r = self.ids.get(id)?;
        unsafe { table::remove_id(self, r) };
        self.ids.remove_id(id);
        Ok(())
    }

    /// Checks if `id` has the `component`.
    #[inline(always)]
    pub fn has<T: HasMeta>(&self, id: Id, key: ComponentKey<T>) -> EcsResult<bool> {
        let r = self.ids.get(id)?;
        let c = component::resolve(self, key)?;
        Ok(self.tables[r.table].col_map.contains(c))
    }

    #[inline]
    pub fn add<T: HasMeta + Default>(&mut self, id: Id, key: ComponentKey<T>) -> EcsResult<()> {
        self.insert(id, key, T::default())
    }

    #[inline]
    pub fn insert<T: HasMeta>(&mut self, id: Id, key: ComponentKey<T>, value: T) -> EcsResult<()> {
        let cid = component::resolve(self, key)?;
        bundle::insert(self, id, (cid, value))?;
        Ok(())
    }

    #[inline]
    pub fn remove<T: HasMeta>(&mut self, id: Id, key: ComponentKey<T>) -> EcsResult<()> {
        let cid = component::resolve(self, key)?;
        let bid = self.bundles.intern([cid].into());
        bundle::remove(self, id, bid)?;
        Ok(())
    }

    #[inline]
    pub fn get<T: HasMeta>(&self, id: Id, key: ComponentKey<T>) -> EcsResult<&T> {
        let r = self.ids.get(id)?;
        let c = component::resolve(self, key)?;
        unsafe { component::get(self, r, c) }.ok_or(EcsError::MissingComponent(id, c))
    }

    #[inline]
    pub fn get_mut<T: HasMeta>(&mut self, id: Id, key: ComponentKey<T>) -> EcsResult<&mut T> {
        let r = self.ids.get(id)?;
        let c = component::resolve(self, key)?;
        unsafe { component::get_mut(self, r, c) }.ok_or(EcsError::MissingComponent(id, c))
    }

    #[inline]
    pub fn relate<T: HasMeta>(&mut self, id: Id, key: &RelationKey, target: Id, payload: T) -> EcsResult<()> {
        let r = relation::resolve(self, key)?;
        unsafe { self.relations[r].relate(id, target, payload)? };
        Ok(())
    }

    #[inline]
    pub fn unrelate<T: HasMeta>(&mut self, id: Id, key: &RelationKey, target: Id) -> EcsResult<()> {
        let r = relation::resolve(self, key)?;
        self.relations[r].unrelate(id, target);
        Ok(())
    }

    #[inline]
    pub fn remove_relation<T: HasMeta>(&mut self, id: Id, key: &RelationKey) -> EcsResult<()> {
        let r = relation::resolve(self, key)?;
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
