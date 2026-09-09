use std::{any::TypeId, marker::PhantomData, ptr::NonNull};

use crate::{
    ComponentId, Follow, Id,
    query::{
        access::private::SealedFetch,
        context::QueryCtx,
        logical::{FollowId, ScopeId},
    },
    table::Table,
};

/// Column index resolved for one select term against one table.
/// [`ColIdx::NONE`] means the table lacks the component, which only an
/// optional term may see.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct ColIdx(u32);

impl ColIdx {
    pub const NONE: Self = Self(u32::MAX);

    #[inline(always)]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    #[inline(always)]
    pub const fn get(self) -> Option<u32> {
        match self.0 {
            u32::MAX => None,
            i => Some(i),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
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
        self.mode == AccessMode::Write
    }

    #[inline]
    pub fn reads(&self) -> bool {
        self.mode == AccessMode::Read
    }
}

/// A base pointer to one table column, tagged with the borrow of the
/// table it came from.
pub struct Col<'a, T> {
    ptr: NonNull<T>,
    marker: PhantomData<&'a [T]>,
}

/// A base pointer to one table column, tagged with the borrow of the
/// table it came from.
pub struct ColMut<'a, T> {
    ptr: NonNull<T>,
    marker: PhantomData<&'a mut [T]>,
}

impl<T> Clone for Col<'_, T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Col<'_, T> {}

impl<T> Clone for ColMut<'_, T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ColMut<'_, T> {}

impl<'a, T> Col<'a, T> {
    #[inline(always)]
    fn new(table: &'a Table, index: u32) -> Self {
        Self {
            ptr: table.column_ptr(index).cast(),
            marker: PhantomData,
        }
    }

    /// # Safety
    /// `row` is within the source table's row count.
    #[inline(always)]
    unsafe fn get(self, row: u32) -> &'a T {
        unsafe { self.ptr.add(row as usize).as_ref() }
    }
}

impl<'a, T> ColMut<'a, T> {
    #[inline(always)]
    fn new(table: &'a Table, index: u32) -> Self {
        Self {
            ptr: table.column_ptr(index).cast(),
            marker: PhantomData,
        }
    }

    /// # Safety
    /// - `row` is within the source table's row count.
    /// - No other reference to this row of this column is live. The plan
    ///   guarantees this across select terms; the iterator guarantees it
    ///   across rows by never revisiting one.
    #[inline(always)]
    unsafe fn get_mut(self, row: u32) -> &'a mut T {
        unsafe { self.ptr.add(row as usize).as_mut() }
    }
}

pub struct TAccess {
    pub type_id: TypeId,
    pub mode: AccessMode,
    pub optional: bool,
}

impl TAccess {
    #[inline(always)]
    pub const fn of<T: 'static>(mode: AccessMode, optional: bool) -> Self {
        Self { type_id: TypeId::of::<T>(), mode, optional }
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
    type Column<'a>: Copy;
    const ACCESS: TAccess;

    /// # Safety
    /// `row` is within the row count of the table `column` came from.
    unsafe fn row<'a>(column: Self::Column<'a>, row: u32) -> Self::Row<'a>;
    fn column(table: &Table, index: ColIdx) -> Self::Column<'_>;
}

impl<T: 'static> TypedAccess for &T {
    type Row<'a> = &'a T;
    type Column<'a> = Col<'a, T>;
    const ACCESS: TAccess = TAccess::of::<T>(AccessMode::Read, false);

    #[inline(always)]
    unsafe fn row<'a>(column: Self::Column<'a>, row: u32) -> Self::Row<'a> {
        unsafe { column.get(row) }
    }

    #[inline(always)]
    fn column(table: &Table, index: ColIdx) -> Self::Column<'_> {
        Col::new(table, index.get().expect("required component missing"))
    }
}

impl<T: 'static> TypedAccess for &mut T {
    type Row<'a> = &'a mut T;
    type Column<'a> = ColMut<'a, T>;
    const ACCESS: TAccess = TAccess::of::<T>(AccessMode::Write, false);

    #[inline(always)]
    unsafe fn row<'a>(column: Self::Column<'a>, row: u32) -> Self::Row<'a> {
        unsafe { column.get_mut(row) }
    }

    #[inline(always)]
    fn column(table: &Table, index: ColIdx) -> Self::Column<'_> {
        ColMut::new(table, index.get().expect("required component missing"))
    }
}

impl<T: 'static> TypedAccess for Option<&T> {
    type Row<'a> = Option<&'a T>;
    type Column<'a> = Option<Col<'a, T>>;
    const ACCESS: TAccess = TAccess::of::<T>(AccessMode::Read, true);

    unsafe fn row<'a>(column: Self::Column<'a>, row: u32) -> Self::Row<'a> {
        column.map(|c| unsafe { c.get(row) })
    }

    fn column(table: &Table, index: ColIdx) -> Self::Column<'_> {
        index.get().map(|i| Col::new(table, i))
    }
}

impl<T: 'static> TypedAccess for Option<&mut T> {
    type Row<'a> = Option<&'a mut T>;
    type Column<'a> = Option<ColMut<'a, T>>;
    const ACCESS: TAccess = TAccess::of::<T>(AccessMode::Write, true);

    unsafe fn row<'a>(column: Self::Column<'a>, row: u32) -> Self::Row<'a> {
        column.map(|c| unsafe { c.get_mut(row) })
    }

    fn column(table: &Table, index: ColIdx) -> Self::Column<'_> {
        index.get().map(|i| ColMut::new(table, i))
    }
}

pub trait Select {
    type Columns<'a>: Copy;
    type Row<'a>;
    const ACCESSES: &[TAccess];

    /// # Safety
    /// `row` is within the row count of the table `columns` came from.
    unsafe fn row<'a>(columns: Self::Columns<'a>, row: u32) -> Self::Row<'a>;
    fn columns<'a>(table: &'a Table, column_indices: &[ColIdx]) -> Self::Columns<'a>;
}

impl Select for () {
    type Columns<'a> = ();
    type Row<'a> = ();
    const ACCESSES: &[TAccess] = &[];

    #[inline(always)]
    unsafe fn row<'a>(_: Self::Columns<'a>, _: u32) -> Self::Row<'a> {}

    #[inline(always)]
    fn columns<'a>(_: &'a Table, _: &[ColIdx]) -> Self::Columns<'a> {}
}

impl<T: TypedAccess> Select for T {
    type Columns<'a> = T::Column<'a>;
    type Row<'a> = T::Row<'a>;
    const ACCESSES: &[TAccess] = &[T::ACCESS];

    #[inline(always)]
    unsafe fn row<'a>(columns: Self::Columns<'a>, row: u32) -> Self::Row<'a> {
        unsafe { T::row(columns, row) }
    }

    #[inline(always)]
    fn columns<'a>(table: &'a Table, column_indices: &[ColIdx]) -> Self::Columns<'a> {
        T::column(table, column_indices[0])
    }
}

macro_rules! impl_column_tuple {
    ($($T:ident),+ $(,)?) => {
        impl<$($T: TypedAccess),+ > Select for ($($T,)+) {
            type Columns<'a> = ($($T::Column<'a>,)+);
            type Row<'a> = ($($T::Row<'a>,)+);
            const ACCESSES: &[TAccess] = &[$($T::ACCESS),*];

            #[inline(always)]
            fn columns<'a>(table: &'a Table, column_indices: &[ColIdx]) -> Self::Columns<'a> {
                let mut i = 0usize;
                ($(
                    #[allow(unused_assignments)]
                    { let col = $T::column(table, column_indices[i]); i += 1; col },
                )+)
            }

            #[inline(always)]
            unsafe fn row<'a>(columns: Self::Columns<'a>, row: u32) -> Self::Row<'a> {
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
