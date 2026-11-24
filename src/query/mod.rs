pub mod access;
pub mod data;
pub mod filter;

use std::marker::PhantomData;

use crate::{
    ecs::Ecs,
    id::{Id, allocator::NotAlive},
    table_storage::TableId,
};

use access::{AccessError, AccessSet};
use data::{QueryData, ReadOnlyData, SliceData};
use filter::QueryFilter;

/// Error returned when fetching one id from a query fails.
#[derive(Debug, thiserror::Error)]
pub enum QueryGetError {
    #[error(transparent)]
    EcsMismatch(#[from] EcsMismatch),
    #[error(transparent)]
    NotAlive(#[from] NotAlive),
    #[error("id does not match the query")]
    NoMatch,
}

#[derive(Debug, thiserror::Error)]
#[error("world mismatch: expected {expected}, got {received}")]
pub struct EcsMismatch {
    pub expected: u32,
    pub received: u32,
}

/// A cached table query fetching `D` from every entity whose table matches
/// `D` and `F`.
///
/// The query does not borrow the [`Ecs`]; it is passed to each run. Tables
/// are never removed, so each run only checks tables created since the last.
pub struct Query<D: QueryData, F: QueryFilter = ()> {
    ecs_id: u32,
    /// Every matched table, with the state `D` resolved for it.
    matched: Vec<(TableId, D::State)>,
    /// For each checked table, its index in `matched` if it matched.
    slots: Vec<Option<u32>>,
    marker: PhantomData<fn() -> F>,
}

impl<D: QueryData, F: QueryFilter> Query<D, F> {
    /// Validates `D`'s access and creates an empty query for `ecs`.
    pub fn new(ecs: &Ecs) -> Result<Self, AccessError> {
        D::access(&mut AccessSet::default())?;
        Ok(Self {
            ecs_id: ecs.ecs_id(),
            matched: vec![],
            slots: vec![],
            marker: PhantomData,
        })
    }

    /// Checks every table created since the last update.
    fn update(&mut self, ecs: &Ecs) -> Result<(), EcsMismatch> {
        if self.ecs_id != ecs.ecs_id() {
            return Err(EcsMismatch { expected: self.ecs_id, received: ecs.ecs_id() });
        }

        let tables = &ecs.storage.tables;
        let seen = self.slots.len();

        ecs.storage.ids(seen).for_each(|id| {
            let table = &tables[id];
            let slot = if F::matches(table) {
                D::resolve(table).map(|state| {
                    let slot = self.matched.len() as u32;
                    self.matched.push((id, state));
                    slot
                })
            } else {
                None
            };
            self.slots.push(slot);
        });

        Ok(())
    }

    /// Calls `f` with the item of every matched row.
    ///
    /// # Safety
    /// The caller holds the access `D` reports for `'w`.
    unsafe fn run<'w>(&self, ecs: &'w Ecs, mut f: impl FnMut(D::Value<'w>)) {
        for &(id, state) in &self.matched {
            let table = &ecs.storage.tables[id];
            let len = table.num_rows() as usize;

            if len == 0 {
                continue;
            }

            // SAFETY: `state` was resolved for this table.
            let fetch = unsafe { D::fetch(state, table) };

            for row in 0..len {
                // SAFETY:
                // - `row < len`
                // - table is unchanged during the run
                // - caller holds `D`'s access.
                f(unsafe { D::fetch_row(fetch, row) });
            }
        }
    }

    /// Calls `f` with the slices of every non-empty matched table.
    ///
    /// # Safety
    /// The caller holds the access `D` reports for `'w`.
    unsafe fn run_tables<'w>(&self, ecs: &'w Ecs, mut f: impl FnMut(D::Slice<'w>))
    where
        D: SliceData,
    {
        for &(id, state) in &self.matched {
            let table = &ecs.storage.tables[id];
            let len = table.num_rows() as usize;

            if len == 0 {
                continue;
            }

            // SAFETY:
            // - `state` was resolved for this table
            // - `len` is its row count
            // - caller holds `D`'s access
            f(unsafe { D::slice(D::fetch(state, table), len) });
        }
    }

    /// The item of `id`.
    ///
    /// # Safety
    /// The caller holds the access `D` reports for `'w`, and this query was
    /// updated against `ecs`.
    unsafe fn get_row<'w>(&self, ecs: &'w Ecs, id: Id) -> Result<D::Value<'w>, QueryGetError> {
        let r = ecs.ids.get(id)?;
        let idx = r.table.index();
        let Some(Some(slot)) = self.slots.get(idx) else { return Err(QueryGetError::NoMatch) };
        let (tid, state) = self.matched[*slot as usize];
        let table = &ecs.storage.tables[tid];
        // SAFETY:
        // - `state` was resolved for this table,
        // - `r.row` is a live row
        // - caller holds `D`'s access.
        Ok(unsafe { D::fetch_row(D::fetch(state, table), r.row as usize) })
    }

    /// Calls `f` with the item of every matched row.
    pub fn each<'w>(&mut self, ecs: &'w mut Ecs, f: impl FnMut(D::Value<'w>)) -> Result<(), EcsMismatch> {
        self.update(ecs)?;
        // SAFETY: `ecs` is borrowed uniquely for `'w`.
        unsafe { self.run(ecs, f) };
        Ok(())
    }

    /// Calls `f` with the item of every matched row, through a shared borrow.
    pub fn read<'w>(&mut self, ecs: &'w Ecs, f: impl FnMut(D::Value<'w>)) -> Result<(), EcsMismatch>
    where
        D: ReadOnlyData,
    {
        self.update(ecs)?;
        // SAFETY: `D` only reads.
        unsafe { self.run(ecs, f) };
        Ok(())
    }

    /// Calls `f` with the column slices of every non-empty matched table.
    pub fn each_table<'w>(&mut self, ecs: &'w mut Ecs, f: impl FnMut(D::Slice<'w>)) -> Result<(), EcsMismatch>
    where
        D: SliceData,
    {
        self.update(ecs)?;
        // SAFETY: `ecs` is borrowed uniquely for `'w`.
        unsafe { self.run_tables(ecs, f) };
        Ok(())
    }

    /// Calls `f` with the column slices of every non-empty matched table,
    /// through a shared borrow.
    pub fn read_table<'w>(&mut self, ecs: &'w Ecs, f: impl FnMut(D::Slice<'w>)) -> Result<(), EcsMismatch>
    where
        D: SliceData + ReadOnlyData,
    {
        self.update(ecs)?;
        // SAFETY: `D` only reads.
        unsafe { self.run_tables(ecs, f) };
        Ok(())
    }

    /// The item of `id`, through a shared borrow.
    pub fn get<'w>(&mut self, ecs: &'w Ecs, id: Id) -> Result<D::Value<'w>, QueryGetError>
    where
        D: ReadOnlyData,
    {
        self.update(ecs)?;
        // SAFETY: `D` only reads, and the query was just updated.
        unsafe { self.get_row(ecs, id) }
    }

    /// The value of `id`.
    pub fn get_mut<'w>(&mut self, ecs: &'w mut Ecs, id: Id) -> Result<D::Value<'w>, QueryGetError> {
        self.update(ecs)?;
        // SAFETY: `ecs` is borrowed uniquely for `'w`, and the query was just updated.
        unsafe { self.get_row(ecs, id) }
    }
}
