use std::sync::atomic::AtomicU32;

use errset::{set, try_};

use crate::{
    AccessError, Field, IdsExhausted, Query, QueryData, QueryFilter,
    bundle::{self, Bundle, BundleRegistry, ComponentSet},
    component::{self, Component, ComponentConfig, ComponentId, ComponentRegisterError, registry::ComponentRegistry},
    error::{EcsResult, Error, MissingComponent},
    id::{
        Id,
        allocator::{IdAllocator, IdRecord, NotAlive},
    },
    table::{self},
    table_storage::TableStorage,
};

fn ecs_id_allocate() -> u32 {
    static NEXT_ID: AtomicU32 = AtomicU32::new(0);
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub struct Ecs {
    ecs_id: u32,
    pub(crate) ids: IdAllocator,
    pub(crate) components: ComponentRegistry,

    pub(crate) bundles: BundleRegistry,
    pub(crate) storage: TableStorage,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            ecs_id: ecs_id_allocate(),
            ids: IdAllocator::new(),
            components: ComponentRegistry::new(),

            bundles: BundleRegistry::new(),
            storage: TableStorage::new(),
        }
    }

    #[inline(always)]
    pub fn ecs_id(&self) -> u32 {
        self.ecs_id
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

    #[inline(always)]
    pub fn read<T: Component>(&self, id: Id) -> Result<Option<&T::Value>, NotAlive> {
        let r = self.ids.get(id)?;
        Ok(unsafe { component::get(self, r, T::id()) })
    }

    #[inline(always)]
    pub fn write<T: Component>(&mut self, id: Id) -> Result<Option<&mut T::Value>, NotAlive> {
        let r = self.ids.get(id)?;
        Ok(unsafe { component::get_mut(self, r, T::id()) })
    }

    /// Creates a query fetching `D` from every matching entity.
    #[inline]
    pub fn query<D: QueryData>(&self) -> Result<Query<D>, AccessError> {
        Query::new(self)
    }

    /// Creates a query fetching `D` from every entity matching `D` and `F`.
    #[inline]
    pub fn query_filtered<D: QueryData, F: QueryFilter>(&self) -> Result<Query<D, F>, AccessError> {
        Query::new(self)
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
