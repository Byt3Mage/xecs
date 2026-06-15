use std::marker::PhantomData;

use crate::{
    Ecs, Error, Id, Query, QueryBuilder, StaticId, TypedStaticId,
    error::EcsResult,
    query::iter::{Field, Row},
};

pub struct TQueryBuilder<'w, T: TRow> {
    builder: QueryBuilder<'w>,
    marker: PhantomData<fn() -> T>,
}

impl<'w, T: TRow> TQueryBuilder<'w, T> {
    pub fn new(ecs: &'w Ecs) -> EcsResult<Self> {
        Ok(Self {
            builder: unsafe { T::add_fields(QueryBuilder::new(ecs)) }?,
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
        TQuery { query: self.builder.build(), marker: PhantomData }
    }
}

pub struct TQuery<T: TRow> {
    query: Query,
    marker: PhantomData<fn() -> T>,
}

impl<T: TRow> TQuery<T> {
    #[inline(always)]
    pub fn each(&self, ecs: &mut Ecs, f: impl FnMut(T::Get<'_>)) {
        self.query.each(ecs, f)
    }
}

pub trait TColumn: Field<RemoveRef: TypedStaticId> {
    unsafe fn add_field(builder: QueryBuilder) -> Result<QueryBuilder, Error>;
}

impl<T: TypedStaticId> TColumn for &T {
    #[inline(always)]
    unsafe fn add_field(builder: QueryBuilder) -> Result<QueryBuilder, Error> {
        builder.read_t::<T>()
    }
}

impl<T: TypedStaticId> TColumn for &mut T {
    #[inline(always)]
    unsafe fn add_field(builder: QueryBuilder) -> Result<QueryBuilder, Error> {
        builder.write_t::<T>()
    }
}

pub trait TRow: Row {
    /// # Safety
    /// Correct fields must be provided for read/write components.
    /// Use the `Row` derive macro for correct implementation.
    unsafe fn add_fields(builder: QueryBuilder) -> Result<QueryBuilder, Error>;
}

impl<T: TypedStaticId> TRow for &T {
    unsafe fn add_fields(builder: QueryBuilder) -> Result<QueryBuilder, Error> {
        builder.read_t::<T>()
    }
}

impl<T: TypedStaticId> TRow for &mut T {
    unsafe fn add_fields(builder: QueryBuilder) -> Result<QueryBuilder, Error> {
        builder.write_t::<T>()
    }
}

macro_rules! impl_tuple_row {
    ($($T:ident),+ $(,)?) => {
        impl<$($T: TColumn),+ > TRow for ($($T,)+) {
            unsafe fn add_fields(mut builder: QueryBuilder) -> Result<QueryBuilder, Error> {
                unsafe { $(builder = $T::add_field(builder)?;)+ }
                 Ok(builder)
            }
        }
    };
}

impl_tuple_row!(P0, P1);
impl_tuple_row!(P0, P1, P2);
impl_tuple_row!(P0, P1, P2, P3);
impl_tuple_row!(P0, P1, P2, P3, P4);
impl_tuple_row!(P0, P1, P2, P3, P4, P5);
impl_tuple_row!(P0, P1, P2, P3, P4, P5, P6);
impl_tuple_row!(P0, P1, P2, P3, P4, P5, P6, P7);
impl_tuple_row!(P0, P1, P2, P3, P4, P5, P6, P7, P8);
impl_tuple_row!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9);
impl_tuple_row!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10);
impl_tuple_row!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11);
impl_tuple_row!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12);
