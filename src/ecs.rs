use std::sync::atomic::AtomicU32;

use crate::{
    AccessError, Bounded, Field, IdsExhausted, Query, QueryData, QueryFilter, RelateError,
    bundle::{self, Bundle, BundleRegistry, ColumnSet, ComponentSet},
    component::{self, Component, ComponentConfig, ComponentId, ComponentRegisterError, registry::ComponentRegistry},
    id::{
        Id,
        allocator::{IdAllocator, IdRecord, NotAlive},
    },
    relation::{self, One, Relation, Sources, Src, TPos, Targets, Tgt, Unbounded, store::RelationStorage},
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
    pub(crate) relations: RelationStorage,
    pub(crate) bundles: BundleRegistry,
    pub(crate) storage: TableStorage,
}

impl Default for Ecs {
    fn default() -> Self {
        Self::new()
    }
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            ecs_id: ecs_id_allocate(),
            ids: IdAllocator::new(),
            components: ComponentRegistry::new(),
            relations: RelationStorage::new(),
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
    pub fn create(&mut self, bundle: impl Bundle<Set: ComponentSet>) -> Result<Id, IdsExhausted> {
        bundle::create(self, |_| bundle)
    }

    /// Creates an [Id] carrying `values`.
    #[inline]
    pub fn create_with_id<B>(&mut self, f: impl FnOnce(Id) -> B) -> Result<Id, IdsExhausted>
    where
        B: Bundle<Set: ComponentSet>,
    {
        bundle::create(self, f)
    }

    /// Creates `n` ids, each holding the bundle `f` returns for it. Nothing is
    /// created unless all `n` ids can be allocated.
    #[inline]
    pub fn batch_create<B>(&mut self, n: u32, f: impl FnMut(u32, Id) -> B) -> Result<(), IdsExhausted>
    where
        B: Bundle<Set: ComponentSet>,
    {
        bundle::batch_create(self, n, f)
    }

    pub fn delete_id(&mut self, id: Id) -> Result<(), NotAlive> {
        // TODO: run remove hooks.
        //TODO: relation::purge(self, id)?;
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
        bundle::insert(self, self.ids.get(id)?, bundle);
        Ok(())
    }

    #[inline(always)]
    pub fn insert<T: Component>(&mut self, id: Id, v: T::Value) -> Result<(), NotAlive> {
        self.insert_bundle(id, Field::<T>::new(v))
    }

    #[inline(always)]
    pub fn remove<S: ColumnSet>(&mut self, id: Id) {
        let Ok(r) = self.ids.get(id) else { return };
        bundle::remove::<S>(self, r);
    }

    #[inline(always)]
    pub fn get<T: Component>(&self, id: Id) -> Result<&T::Value, EcsGetError> {
        let r = self.ids.get(id)?;
        let comp = T::id();
        unsafe { component::get(&self.storage.tables, r, comp) }.ok_or(EcsGetError::NoComponent { id, comp })
    }

    #[inline(always)]
    pub fn get_mut<T: Component>(&mut self, id: Id) -> Result<&mut T::Value, EcsGetError> {
        let r = self.ids.get(id)?;
        let comp = T::id();
        unsafe { component::get_mut(&mut self.storage.tables, r, comp) }.ok_or(EcsGetError::NoComponent { id, comp })
    }

    /// Creates the edge `src -> tgt` of `R` carrying `data`, replacing whatever
    /// edges `R`'s cardinalities require. An existing `src -> tgt` keeps its
    /// position and takes `data`. For a symmetric `R` the edge joins `src` and `tgt`
    /// in both directions.
    #[inline]
    pub fn relate<R>(&mut self, src: Id, tgt: Id, data: R::Data) -> Result<(), RelateError<R>>
    where
        R: Relation<Targets: Unbounded>,
    {
        Src::<R>::relate::<R>(self, src, tgt, data)
    }

    /// Creates the edge `src -> tgt` of `R` at `pos` carrying `data`. It replaces whatever
    /// edges `R`'s cardinalities require. An existing `src -> tgt` keeps its
    /// position and takes `data`. For a symmetric `R` the edge joins `src` and `tgt`
    /// in both directions.
    #[inline]
    pub fn relate_at<R>(&mut self, src: Id, tgt: Id, pos: TPos<R>, data: R::Data) -> Result<(), RelateError<R>>
    where
        R: Relation<Targets: Bounded>,
    {
        Src::<R>::relate_at::<R>(self, src, tgt, pos, data)
    }

    /// Removes the edge `s -> t` of `R`. Does nothing when it is absent.
    #[inline]
    pub fn unrelate<R: Relation>(&mut self, src: Id, tgt: Id) {
        Src::<R>::unrelate::<R>(self, src, tgt);
    }

    /// Removes every edge of `R` out of `s`. Does nothing when there are none.
    #[inline]
    pub fn clear<R: Relation>(&mut self, s: Id) -> Result<(), NotAlive> {
        todo!()
    }

    /// The target of `s`'s edge of `R`, if it has one.
    #[inline]
    pub fn target<R: Relation<Targets = One>>(&self, s: Id) -> Result<Option<Id>, NotAlive> {
        let r = self.ids.get(s)?;
        Ok(relation::row::<R>(self, r).map(|tgt| tgt.id))
    }

    /// The target of `s`'s edge of `R`, if it has one.
    #[inline]
    pub fn targets<R: Relation>(&self, s: Id, mut f: impl FnMut(Id, &R::Data)) -> Result<(), NotAlive> {
        let r = self.ids.get(s)?;
        Tgt::<R>::for_each::<R>(self, r, |t| f(t.id, &t.data));
        Ok(())
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
    pub fn live_count(&self) -> u32 {
        self.ids.num_live()
    }

    #[inline(always)]
    pub fn dead_count(&self) -> u32 {
        self.ids.num_dead()
    }
}

#[derive(Debug, thiserror::Error, Clone, Copy)]
pub enum EcsGetError {
    #[error(transparent)]
    NotAlive(#[from] NotAlive),
    #[error("{id} does not have component {comp}")]
    NoComponent { id: Id, comp: ComponentId },
}
