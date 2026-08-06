use std::{any::TypeId, ptr::NonNull};

use crate::{
    ComponentId, Follow, Id,
    inline_vec::InlineVec,
    invec,
    query::{
        access::private::SealedFetch,
        context::QueryCtx,
        logical::{FollowId, ScopeId},
    },
    storage::table::Table,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    Read,
    Write,
}

/// One SELECT term: data access with a mode
#[derive(Debug, Clone, Copy)]
pub struct Access {
    pub id: ComponentId,
    pub mode: AccessMode,
    pub optional: bool,
}

impl Access {
    #[inline]
    pub fn writes(&self) -> bool {
        matches!(self.mode, AccessMode::Write)
    }

    #[inline]
    pub fn reads(&self) -> bool {
        matches!(self.mode, AccessMode::Read)
    }
}

pub struct TypedAccessDesc {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub mode: AccessMode,
    pub optional: bool,
}

impl TypedAccessDesc {
    #[inline(always)]
    pub fn of<T: 'static>(mode: AccessMode, optional: bool) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
            mode,
            optional,
        }
    }
}

mod private {
    pub trait SealedFetch {}
}

impl SealedFetch for Id {}
impl<T> SealedFetch for &T {}
impl<T> SealedFetch for &mut T {}
impl<T> SealedFetch for Option<&T> {}
impl<T> SealedFetch for Option<&mut T> {}

pub trait TypedAccess: SealedFetch {
    type Row<'a>;
    type Column: Copy;
    type ColumnIndex;

    fn describe() -> TypedAccessDesc;
    fn column_index(raw: usize) -> Self::ColumnIndex;
    fn column(table: &Table, index: Self::ColumnIndex) -> Self::Column;
    unsafe fn row<'a>(column: Self::Column, row: u32) -> Self::Row<'a>;
}

impl<T: 'static> TypedAccess for &T {
    type Row<'a> = &'a T;
    type Column = NonNull<T>;
    type ColumnIndex = usize;

    fn describe() -> TypedAccessDesc {
        TypedAccessDesc::of::<T>(AccessMode::Read, false)
    }

    #[inline(always)]
    fn column_index(raw: usize) -> Self::ColumnIndex {
        raw
    }

    #[inline(always)]
    fn column(table: &Table, index: Self::ColumnIndex) -> Self::Column {
        table.column(index).data.ptr().cast()
    }

    #[inline(always)]
    unsafe fn row<'a>(column: Self::Column, row: u32) -> Self::Row<'a> {
        unsafe { column.add(row as usize).as_ref() }
    }
}

impl<T: 'static> TypedAccess for &mut T {
    type Row<'a> = &'a mut T;
    type Column = NonNull<T>;
    type ColumnIndex = usize;

    fn describe() -> TypedAccessDesc {
        TypedAccessDesc::of::<T>(AccessMode::Write, false)
    }

    #[inline(always)]
    fn column_index(raw: usize) -> Self::ColumnIndex {
        raw
    }

    #[inline(always)]
    fn column(table: &Table, index: Self::ColumnIndex) -> Self::Column {
        table.column(index).data.ptr().cast()
    }

    #[inline(always)]
    unsafe fn row<'a>(column: Self::Column, row: u32) -> Self::Row<'a> {
        unsafe { column.add(row as usize).as_mut() }
    }
}

impl<T: 'static> TypedAccess for Option<&T> {
    type Row<'a> = Option<&'a T>;
    type Column = Option<NonNull<T>>;
    type ColumnIndex = Option<usize>;

    fn describe() -> TypedAccessDesc {
        TypedAccessDesc::of::<T>(AccessMode::Read, true)
    }

    fn column_index(raw: usize) -> Self::ColumnIndex {
        (raw != usize::MAX).then_some(raw)
    }

    fn column(table: &Table, index: Self::ColumnIndex) -> Self::Column {
        index.map(|i| table.column(i).data.ptr().cast())
    }

    unsafe fn row<'a>(column: Self::Column, row: u32) -> Self::Row<'a> {
        column.map(|c| unsafe { c.add(row as usize).as_ref() })
    }
}

impl<T: 'static> TypedAccess for Option<&mut T> {
    type Row<'a> = Option<&'a mut T>;
    type Column = Option<NonNull<T>>;
    type ColumnIndex = Option<usize>;

    fn describe() -> TypedAccessDesc {
        TypedAccessDesc::of::<T>(AccessMode::Write, true)
    }

    fn column_index(raw: usize) -> Self::ColumnIndex {
        (raw != usize::MAX).then_some(raw)
    }

    fn column(table: &Table, index: Self::ColumnIndex) -> Self::Column {
        index.map(|i| table.column(i).data.ptr().cast())
    }

    unsafe fn row<'a>(column: Self::Column, row: u32) -> Self::Row<'a> {
        column.map(|c| unsafe { c.add(row as usize).as_mut() })
    }
}

pub trait Select {
    type Columns: Copy;
    type Row<'a>;

    fn describe() -> InlineVec<TypedAccessDesc, 4>;
    fn columns(table: &Table, column_indices: &[usize]) -> Self::Columns;
    unsafe fn row<'a>(columns: Self::Columns, row: u32) -> Self::Row<'a>;
}

impl Select for () {
    type Columns = ();
    type Row<'a> = ();

    fn describe() -> InlineVec<TypedAccessDesc, 4> {
        invec![]
    }

    #[inline(always)]
    fn columns(_: &Table, _: &[usize]) -> Self::Columns {}

    #[inline(always)]
    unsafe fn row<'a>(_: Self::Columns, _: u32) -> Self::Row<'a> {}
}

impl<T: TypedAccess> Select for T {
    type Columns = T::Column;
    type Row<'a> = T::Row<'a>;

    fn describe() -> InlineVec<TypedAccessDesc, 4> {
        invec![T::describe()]
    }

    #[inline(always)]
    fn columns(table: &Table, column_indices: &[usize]) -> Self::Columns {
        T::column(table, T::column_index(column_indices[0]))
    }

    #[inline(always)]
    unsafe fn row<'a>(columns: Self::Columns, row: u32) -> Self::Row<'a> {
        unsafe { T::row(columns, row) }
    }
}

pub trait Follows {
    type Get<'a>;
    fn get<'a>(ctx: &'a QueryCtx<'a>, scope: ScopeId, from: Id, indices: &[FollowId]) -> Self::Get<'a>;
}

impl Follows for () {
    type Get<'a> = ();
    fn get<'a>(_: &'a QueryCtx<'a>, _: ScopeId, _: Id, _: &[FollowId]) -> Self::Get<'a> {}
}

impl<F: Follows> Follows for Follow<'_, F> {
    type Get<'a> = Follow<'a, F>;

    fn get<'a>(ctx: &'a QueryCtx<'a>, scope: ScopeId, from: Id, indices: &[FollowId]) -> Self::Get<'a> {
        ctx.binds.set(scope, from);
        Follow::new(ctx, from, indices[0])
    }
}

macro_rules! impl_column_tuple {
    ($($T:ident),+ $(,)?) => {
        impl<$($T: TypedAccess),+ > Select for ($($T,)+) {
            type Columns = ($($T::Column,)+);
            type Row<'a> = ($($T::Row<'a>,)+);

            fn describe() -> InlineVec<TypedAccessDesc, 4> {
                invec![$($T::describe(),)+]
            }

            #[inline(always)]
            fn columns(table: &Table, column_indices: &[usize]) -> Self::Columns {
                let mut i = 0usize;
                ($(
                    #[allow(unused_assignments)]
                    { let col =  $T::column(table, $T::column_index(column_indices[i])); i += 1; col },
                )+)
            }

            unsafe fn row<'a>(columns: Self::Columns, row: u32) -> Self::Row<'a> {
                #[allow(non_snake_case)]
                let ($($T,)+) = columns;
                unsafe { ($($T::row($T, row),)+) }
            }
        }
    };
}
impl_column_tuple!(P0, P1);
impl_column_tuple!(P0, P1, P2);
impl_column_tuple!(P0, P1, P2, P3);
impl_column_tuple!(P0, P1, P2, P3, P4);
impl_column_tuple!(P0, P1, P2, P3, P4, P5);
impl_column_tuple!(P0, P1, P2, P3, P4, P5, P6);
impl_column_tuple!(P0, P1, P2, P3, P4, P5, P6, P7);
impl_column_tuple!(P0, P1, P2, P3, P4, P5, P6, P7, P8);
impl_column_tuple!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9);
impl_column_tuple!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10);
impl_column_tuple!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11);
impl_column_tuple!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12);
