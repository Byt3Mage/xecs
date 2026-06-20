use std::rc::Rc;

use smallvec::SmallVec;

use crate::{
    access::{Access, AccessList, AccessType},
    component::{StaticId, TypedStaticId},
    ecs::Ecs,
    error::EcsResult,
    id::Id,
    query::{fetch::ComponentFetch, iter::Columns},
    storage::Storage,
    table_index::TableId,
    validate::WriteAccessError,
};

use self::iter::TableIter;

pub mod dsl;
pub mod fetch;
pub mod iter;
pub mod typed_query;

struct TableMatch {
    table_id: TableId,
    col_indices: Box<[usize]>,
}

pub struct QueryBuilder<'w> {
    pub(crate) ecs: &'w Ecs,
    pub(crate) fields: AccessList,
    pub(crate) singletons: AccessList,
    pub(crate) with: Vec<Id>,
    pub(crate) without: Vec<Id>,
}

impl<'w> QueryBuilder<'w> {
    pub fn new(ecs: &'w Ecs) -> Self {
        Self {
            ecs,
            fields: AccessList::new(),
            singletons: AccessList::new(),
            with: vec![],
            without: vec![],
        }
    }

    #[inline]
    pub fn fetch(mut self, access: Access) -> Result<Self, WriteAccessError> {
        self.fields.push(access)?;
        Ok(self)
    }

    #[inline]
    pub fn fetch_t<T>(self) -> EcsResult<Self>
    where
        T: ComponentFetch,
        T::RemoveRef: TypedStaticId,
    {
        let id = self.ecs.id_t::<T::RemoveRef>()?;
        let ty = T::ACCESS_TYPE;
        Ok(self.fetch(Access { id, ty })?)
    }

    pub fn read_id(self, id: Id) -> Result<Self, WriteAccessError> {
        self.fetch(Access { id, ty: AccessType::Read })
    }

    pub fn write_id(self, id: Id) -> Result<Self, WriteAccessError> {
        self.fetch(Access { id, ty: AccessType::Write })
    }

    pub fn with_id(mut self, id: Id) -> Self {
        self.with.push(id);
        self
    }

    pub fn without_id(mut self, id: Id) -> Self {
        self.without.push(id);
        self
    }

    pub fn read<T>(self, comp: &StaticId<T>) -> EcsResult<Self> {
        let id = self.ecs.id(comp)?;
        Ok(self.read_id(id)?)
    }

    pub fn write<T>(self, comp: &StaticId<T>) -> EcsResult<Self> {
        let id = self.ecs.id(comp)?;
        Ok(self.write_id(id)?)
    }

    pub fn with<T>(self, comp: &StaticId<T>) -> EcsResult<Self> {
        let id = self.ecs.id(comp)?;
        Ok(self.with_id(id))
    }

    pub fn without<T>(self, comp: &StaticId<T>) -> EcsResult<Self> {
        let id = self.ecs.id(comp)?;
        Ok(self.without_id(id))
    }

    pub fn with_t<T: TypedStaticId>(self) -> EcsResult<Self> {
        let id = self.ecs.id_t::<T>()?;
        Ok(self.with_id(id))
    }

    pub fn without_t<T: TypedStaticId>(self) -> EcsResult<Self> {
        let id = self.ecs.id_t::<T>()?;
        Ok(self.without_id(id))
    }

    pub fn build(self) -> Query {
        let mut query = Query {
            fields: self.fields.into(),
            resources: self.singletons.into(),
            with: self.with.into(),
            without: self.without.into(),
            matches: vec![],
        };

        query.match_tables(self.ecs);
        query
    }
}

pub struct Query {
    fields: AccessList,
    resources: AccessList,
    with: Rc<[Id]>,
    without: Rc<[Id]>,
    matches: Vec<TableMatch>,
}

impl Query {
    pub fn builder(ecs: &Ecs) -> QueryBuilder<'_> {
        QueryBuilder::new(ecs)
    }

    fn try_match_all(&mut self, ecs: &Ecs) -> bool {
        if !(self.fields.is_empty() && self.with.is_empty()) {
            return false;
        }

        self.matches = ecs
            .tables
            .all_table_ids()
            .filter_map(|&t| match self.without.iter().any(|&id| ecs.tables[t].sig.has(id)) {
                true => None,
                false => Some(TableMatch { table_id: t, col_indices: Box::new([]) }),
            })
            .collect();

        true
    }

    fn match_tables(&mut self, ecs: &Ecs) {
        self.matches.clear();

        if self.try_match_all(ecs) {
            return;
        }

        let required: SmallVec<[Id; 8]> = self
            .fields
            .iter()
            .map(|f| f.id)
            .chain(self.with.iter().copied())
            .collect();

        let mut smallest = None;
        let mut smallest_len = usize::MAX;

        for &id in &required {
            if let Storage::Tables(tables) = &ecs.components[id].storage
                && tables.len() < smallest_len
            {
                smallest_len = tables.len();
                smallest = Some(tables);
            }
        }

        // required can't be empty here
        let smallest = smallest.unwrap();

        for &table_id in smallest {
            let table = &ecs.tables[table_id];

            // Must have all required ids
            // AND
            // Must NOT have any excluded ids
            if required.iter().all(|&id| table.sig.has(id)) && !self.without.iter().any(|&id| table.sig.has(id)) {
                // Resolve column indices for each field
                let columns = self
                    .fields
                    .iter()
                    .map(|f| table.col_map.get(f.id).copied().unwrap_or(usize::MAX))
                    .collect();
                self.matches.push(TableMatch { table_id, col_indices: columns });
            }
        }
    }

    /// Internal: wrap this query against a shared `&Ecs`.
    #[inline]
    fn view<'a>(&'a self, ecs: &'a Ecs) -> QueryView<'a> {
        QueryView { query: self, ecs }
    }

    #[inline]
    pub fn each<T: Columns>(&self, ecs: &mut Ecs, f: impl FnMut(T::Row<'_>)) {
        // &mut Ecs is the gate: forces multi-query access through `combine`.
        self.view(ecs).each::<T>(f)
    }

    #[inline]
    pub fn each_table(&self, ecs: &mut Ecs, f: impl FnMut(TableIter<'_>)) {
        self.view(ecs).each_table(f)
    }

    #[inline]
    pub fn try_each_table<F, E>(&self, ecs: &mut Ecs, f: F) -> Result<(), E>
    where
        F: FnMut(TableIter<'_>) -> Result<(), E>,
    {
        self.view(ecs).try_each_table(f)
    }
}

/// A single query bound to a shared `&Ecs`, handed out by `CombinedQuery::run`.
/// Carries no `&mut` — exclusivity across the combined set was proven by
/// `combine`'s `check_disjoint`, so multiple views coexist over one `&Ecs`.
pub struct QueryView<'a> {
    query: &'a Query,
    ecs: &'a Ecs,
}

impl<'a> QueryView<'a> {
    #[inline]
    fn tables(&self) -> impl Iterator<Item = TableIter<'a>> {
        let ecs = self.ecs;
        let query = self.query;
        query.matches.iter().map(move |entry| TableIter {
            ecs,
            table: &ecs.tables[entry.table_id],
            col_indices: &entry.col_indices,
            fields: &query.fields,
            singletons: &query.resources,
        })
    }

    #[inline]
    pub fn each<T: Columns>(&self, mut f: impl FnMut(T::Row<'_>)) {
        self.tables().for_each(|t| t.each_row::<T>(|r| f(r)))
    }

    #[inline]
    pub fn each_table(&self, f: impl FnMut(TableIter<'_>)) {
        self.tables().for_each(f)
    }

    #[inline]
    pub fn try_each_table<F, E>(&self, f: F) -> Result<(), E>
    where
        F: FnMut(TableIter<'_>) -> Result<(), E>,
    {
        self.tables().try_for_each(f)
    }
}

pub struct CombinedQuery<'q, const N: usize> {
    queries: [&'q Query; N],
}

impl<'q, const N: usize> CombinedQuery<'q, N> {
    /// Run a closure with N query views over a shared `&Ecs`. The `&mut Ecs`
    /// gate guarantees no other query runs concurrently; `combine` proved these
    /// N are pairwise disjoint, so the views may be used together freely.
    #[inline]
    pub fn run<R>(&self, ecs: &mut Ecs, f: impl FnOnce([QueryView<'_>; N]) -> R) -> R {
        let ecs: &Ecs = ecs; // one &mut → N shared views
        let views = self.queries.map(|q| QueryView { query: q, ecs });
        f(views)
    }
}

pub fn combine<const N: usize>(queries: [&Query; N]) -> Result<CombinedQuery<'_, N>, WriteAccessError> {
    let fields: [&[Access]; N] = queries.map(|q| &*q.fields);
    let resources: [&[Access]; N] = queries.map(|q| &*q.resources);

    crate::validate::check_combined(&fields)?;
    crate::validate::check_combined(&resources)?;

    Ok(CombinedQuery { queries })
}
