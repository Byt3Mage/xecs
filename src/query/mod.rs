use smallvec::SmallVec;

use crate::{
    component::{StaticId, TypedStaticId},
    ecs::Ecs,
    error::EcsResult,
    id::Id,
    storage::Storage,
    table_index::TableId,
};

use self::iter::TableIter;

pub mod dsl;
pub mod iter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read(Id),
    Write(Id),
}

impl Access {
    fn id(&self) -> Id {
        match self {
            Access::Read(id) | Access::Write(id) => *id,
        }
    }
}

struct TableMatch {
    table_id: TableId,
    columns: Box<[usize]>,
}

pub struct Query {
    access: Box<[Access]>,
    with: Box<[Id]>,
    without: Box<[Id]>,
    matches: Vec<TableMatch>,
}

impl Query {
    pub fn builder(ecs: &Ecs) -> QueryBuilder<'_> {
        QueryBuilder::new(ecs)
    }

    fn try_match_all(&mut self, ecs: &Ecs) -> bool {
        if !(self.access.is_empty() && self.with.is_empty()) {
            return false;
        }

        self.matches = ecs
            .tables
            .all_table_ids()
            .filter_map(|&t| match self.without.iter().any(|&id| ecs.tables[t].sig.has(id)) {
                true => None,
                false => Some(TableMatch { table_id: t, columns: Box::new([]) }),
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
            .access
            .iter()
            .map(|a| a.id())
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
                let columns = self.access.iter().map(|f| table.col_map[f.id()]).collect();
                self.matches.push(TableMatch { table_id, columns });
            }
        }
    }

    /// Iterate all matched tables, calling `f` for each one.
    /// The callback receives a `TableIter` to extract column slices.
    pub fn each_table(&self, ecs: &Ecs, mut f: impl FnMut(TableIter<'_>)) {
        for entry in &self.matches {
            f(TableIter {
                ecs,
                table: &ecs.tables[entry.table_id],
                col_indices: &entry.columns,
                singletons: todo!(),
            });
        }
    }

    /// Iterate all matched tables, calling `f` for each one.
    /// The callback receives a `TableIter` to extract column slices.
    pub fn try_each_table<F, E>(&self, ecs: &Ecs, mut f: F) -> Result<(), E>
    where
        F: FnMut(TableIter<'_>) -> Result<(), E>,
    {
        Ok(for entry in &self.matches {
            f(TableIter {
                ecs,
                table: &ecs.tables[entry.table_id],
                col_indices: &entry.columns,
                singletons: todo!(),
            })?;
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QueryBuildError {
    #[error("query write conflict: {0}")]
    WriteConflict(Id),
}

pub struct QueryBuilder<'w> {
    ecs: &'w Ecs,
    columns: Vec<Access>,
    with: Vec<Id>,
    without: Vec<Id>,
}

impl<'w> QueryBuilder<'w> {
    pub fn new(ecs: &'w Ecs) -> Self {
        Self {
            ecs,
            columns: vec![],
            with: vec![],
            without: vec![],
        }
    }

    pub fn read_id(mut self, id: Id) -> Self {
        self.columns.push(Access::Read(id));
        self
    }

    pub fn write_id(mut self, id: Id) -> Self {
        self.columns.push(Access::Write(id));
        self
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
        Ok(self.read_id(id))
    }

    pub fn write<T>(self, comp: &StaticId<T>) -> EcsResult<Self> {
        let id = self.ecs.id(comp)?;
        Ok(self.write_id(id))
    }

    pub fn with<T>(self, comp: &StaticId<T>) -> EcsResult<Self> {
        let id = self.ecs.id(comp)?;
        Ok(self.with_id(id))
    }

    pub fn without<T>(self, comp: &StaticId<T>) -> EcsResult<Self> {
        let id = self.ecs.id(comp)?;
        Ok(self.without_id(id))
    }

    pub fn read_t<T: TypedStaticId>(self) -> EcsResult<Self> {
        let id = self.ecs.id_t::<T>()?;
        Ok(self.read_id(id))
    }

    pub fn write_t<T: TypedStaticId>(self) -> EcsResult<Self> {
        let id = self.ecs.id_t::<T>()?;
        Ok(self.write_id(id))
    }

    pub fn with_t<T: TypedStaticId>(self) -> EcsResult<Self> {
        let id = self.ecs.id_t::<T>()?;
        Ok(self.with_id(id))
    }

    pub fn without_t<T: TypedStaticId>(self) -> EcsResult<Self> {
        let id = self.ecs.id_t::<T>()?;
        Ok(self.without_id(id))
    }

    pub fn build(self) -> Result<Query, QueryBuildError> {
        // Validate no conflicting access on the same id
        validate_access(&self.columns).map_err(QueryBuildError::WriteConflict)?;

        let mut query = Query {
            access: self.columns.into(),
            with: self.with.into(),
            without: self.without.into(),
            matches: vec![],
        };

        query.match_tables(self.ecs);
        Ok(query)
    }
}

#[inline]
fn validate_access(access_list: &[Access]) -> Result<(), Id> {
    let len = access_list.len();

    for i in 0..len {
        for j in (i + 1)..len {
            let a = access_list[i];
            let b = access_list[j];

            if let (Access::Write(a), Access::Write(b)) = (a, b)
                && a == b
            {
                return Err(a);
            }
        }
    }

    Ok(())
}
