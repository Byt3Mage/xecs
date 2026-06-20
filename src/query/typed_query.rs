use std::marker::PhantomData;

use crate::{
    Ecs, Id, Query, QueryBuilder,
    component::{StaticId, TypedStaticId},
    error::EcsResult,
    query::iter::Columns,
};

pub struct TQueryBuilder<'w, T: Columns> {
    builder: QueryBuilder<'w>,
    marker: PhantomData<fn() -> T>,
}

impl<'w, T: Columns> TQueryBuilder<'w, T> {
    pub fn new(ecs: &'w Ecs) -> EcsResult<Self> {
        Ok(Self {
            builder: QueryBuilder::new(ecs),
            marker: PhantomData,
        })
    }

    pub fn with_id(self, id: Id) -> Self {
        Self {
            builder: self.builder.with_id(id),
            marker: PhantomData,
        }
    }

    pub fn without_id(self, id: Id) -> Self {
        Self {
            builder: self.builder.without_id(id),
            marker: PhantomData,
        }
    }

    pub fn with<C>(self, comp: &StaticId<C>) -> EcsResult<Self> {
        self.builder
            .with(comp)
            .map(|builder| Self { builder, marker: PhantomData })
    }

    pub fn without<C>(self, comp: &StaticId<C>) -> EcsResult<Self> {
        self.builder
            .without(comp)
            .map(|builder| Self { builder, marker: PhantomData })
    }

    pub fn with_t<C: TypedStaticId>(self) -> EcsResult<Self> {
        self.builder
            .with_t::<C>()
            .map(|builder| Self { builder, marker: PhantomData })
    }

    pub fn without_t<C: TypedStaticId>(self) -> EcsResult<Self> {
        self.builder
            .without_t::<C>()
            .map(|builder| Self { builder, marker: PhantomData })
    }

    pub fn build(self) -> TQuery<T> {
        crate::validate::check_row(T::ACCESSES, &self.builder.fields);
        TQuery { query: self.builder.build(), marker: PhantomData }
    }
}

pub struct TQuery<T: Columns> {
    query: Query,
    marker: PhantomData<fn() -> T>,
}

impl<T: Columns> TQuery<T> {
    #[inline(always)]
    pub fn each(&self, ecs: &mut Ecs, f: impl FnMut(T::Row<'_>)) {
        self.query.each(ecs, f)
    }
}
