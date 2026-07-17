use std::ptr::NonNull;

use crate::{query::fetch::private::SealedFetch, storage::table::Table};

mod private {
    pub trait SealedFetch {}
}

impl<T> SealedFetch for &T {}
impl<T> SealedFetch for &mut T {}
impl<T: SealedFetch> SealedFetch for Option<T> {}

pub trait ComponentFetch: SealedFetch {
    type Row<'a>;
    type Column: Copy;
    type ColumnIndex;

    unsafe fn row<'a>(column: Self::Column, row: u32) -> Self::Row<'a>;
    fn column(table: &Table, index: Self::ColumnIndex) -> Self::Column;
    fn column_index(raw: usize) -> Self::ColumnIndex;
}

impl<T: 'static> ComponentFetch for &T {
    type Row<'a> = &'a T;
    type Column = NonNull<T>;
    type ColumnIndex = usize;

    #[inline(always)]
    unsafe fn row<'a>(column: Self::Column, row: u32) -> Self::Row<'a> {
        unsafe { column.add(row as usize).as_ref() }
    }

    #[inline(always)]
    fn column(table: &Table, index: Self::ColumnIndex) -> Self::Column {
        table.data.columns()[index].data.ptr().cast()
    }

    #[inline(always)]
    fn column_index(raw: usize) -> Self::ColumnIndex {
        raw
    }
}

impl<T: 'static> ComponentFetch for &mut T {
    type Row<'a> = &'a mut T;
    type Column = NonNull<T>;
    type ColumnIndex = usize;

    #[inline(always)]
    unsafe fn row<'a>(column: Self::Column, row: u32) -> Self::Row<'a> {
        unsafe { column.add(row as usize).as_mut() }
    }

    #[inline(always)]
    fn column(table: &Table, index: Self::ColumnIndex) -> Self::Column {
        table.data.columns()[index].data.ptr().cast()
    }

    #[inline(always)]
    fn column_index(raw: usize) -> Self::ColumnIndex {
        raw
    }
}

impl<T: ComponentFetch> ComponentFetch for Option<T> {
    type Row<'a> = Option<T::Row<'a>>;
    type Column = Option<T::Column>;
    type ColumnIndex = Option<T::ColumnIndex>;

    unsafe fn row<'a>(column: Self::Column, row: u32) -> Self::Row<'a> {
        column.map(|c| unsafe { T::row(c, row) })
    }

    fn column(table: &Table, index: Self::ColumnIndex) -> Self::Column {
        index.map(|i| T::column(table, i))
    }

    fn column_index(raw: usize) -> Self::ColumnIndex {
        (raw != usize::MAX).then(|| T::column_index(raw))
    }
}
