use crate::{
    Field, IdsExhausted, RelationKey,
    bundle::{self, Bundle, BundleRegistry, ComponentSet},
    component::{self, Component, ComponentConfig, ComponentId, ComponentRegisterError, registry::ComponentRegistry},
    error::{EcsError, EcsResult},
    id::{
        Id,
        allocator::{IdAllocator, IdRecord, NotAlive},
    },
    relation::{self, RelationRegisterError, RelationRegistry, id::RelationId},
    table::{self},
    table_storage::TableStorage,
    type_meta::HasMeta,
};

pub struct Ecs {
    pub(crate) ids: IdAllocator,
    pub(crate) components: ComponentRegistry,
    pub(crate) relations: RelationRegistry,
    pub(crate) bundles: BundleRegistry,
    pub(crate) storage: TableStorage,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            ids: IdAllocator::new(),
            components: ComponentRegistry::new(),
            relations: RelationRegistry::new(),
            bundles: BundleRegistry::new(),
            storage: TableStorage::new(),
        }
    }

    #[inline(always)]
    pub fn generation(&self) -> u64 {
        self.storage.generation()
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

    /// Creates a new [Id] with no components.
    #[inline]
    pub fn empty(&mut self) -> Result<Id, IdsExhausted> {
        self.ids.new_id(|id| {
            let table = TableStorage::ROOT;
            let root = &mut self.storage.tables[table];
            root.data.reserve(1);
            // SAFETY: one row is reserved, and the root table has no columns
            IdRecord { table, row: unsafe { root.data.push_row(id) } }
        })
    }

    /// Creates an [Id] carrying `values`.
    #[inline]
    pub fn create(&mut self, bundle: impl Bundle) -> Result<Id, IdsExhausted> {
        bundle::create(self, |_| bundle)
    }

    /// Creates an [Id] carrying `values`.
    #[inline]
    pub fn create_with_id<B: Bundle>(&mut self, f: impl FnOnce(Id) -> B) -> Result<Id, IdsExhausted> {
        bundle::create(self, f)
    }

    /// Creates `n` ids, each holding the bundle `f` returns for it. Nothing is
    /// created unless all `n` ids can be allocated.
    #[inline]
    pub fn batch_create<B: Bundle>(&mut self, n: u32, f: impl FnMut(u32, Id) -> B) -> Result<(), IdsExhausted> {
        bundle::batch_create(self, n, f)
    }

    pub fn delete_id(&mut self, id: Id) -> Result<(), NotAlive> {
        // TODO: run remove hooks.
        let r = self.ids.get(id)?;
        table::remove_id(self, r);
        self.ids.remove_id(id);
        Ok(())
    }

    /// Checks if `id` has the `component`.
    #[inline(always)]
    pub fn has<T: Component>(&self, id: Id) -> Result<bool, NotAlive> {
        let r = self.ids.get(id)?;
        Ok(self.storage.tables[r.table].has_component(T::id()))
    }

    #[inline(always)]
    pub fn insert_bundle(&mut self, id: Id, bundle: impl Bundle) -> Result<(), NotAlive> {
        bundle::insert(self, id, bundle)
    }

    #[inline(always)]
    pub fn insert<T: Component>(&mut self, id: Id, v: T::Value) -> Result<(), NotAlive> {
        self.insert_bundle(id, Field::<T>::new(v))
    }

    #[inline(always)]
    pub fn remove<S: ComponentSet>(&mut self, id: Id) -> Result<(), NotAlive> {
        bundle::remove::<S>(self, id)
    }

    #[inline]
    pub fn read<T: Component>(&self, id: Id) -> EcsResult<&T::Value> {
        let r = self.ids.get(id)?;
        let c = T::id();
        unsafe { component::get(self, r, c) }.ok_or(EcsError::MissingComponent(id, c))
    }

    #[inline]
    pub fn write<T: Component>(&mut self, id: Id) -> EcsResult<&mut T::Value> {
        let r = self.ids.get(id)?;
        let c = T::id();
        unsafe { component::get_mut(self, r, c) }.ok_or(EcsError::MissingComponent(id, c))
    }

    #[inline]
    pub fn relate<T: HasMeta>(&mut self, id: Id, key: &RelationKey, target: Id, payload: T) -> EcsResult<()> {
        let r = relation::resolve(self, key).unwrap();
        unsafe { self.relations[r].relate(id, target, payload)? };
        Ok(())
    }

    #[inline]
    pub fn unrelate<T: HasMeta>(&mut self, id: Id, key: &RelationKey, target: Id) -> EcsResult<()> {
        let r = relation::resolve(self, key).unwrap();
        self.relations[r].unrelate(id, target);
        Ok(())
    }

    #[inline]
    pub fn remove_relation<T: HasMeta>(&mut self, id: Id, key: &RelationKey) -> EcsResult<()> {
        let r = relation::resolve(self, key).unwrap();
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
