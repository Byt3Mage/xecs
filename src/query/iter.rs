use crate::{
    Ecs,
    access::AccessType,
    id::Id,
    query::{
        Access,
        fetch::{ComponentFetch, ReadOnlyFetch},
    },
    storage::table::Table,
};

pub struct TableIter<'a> {
    pub(super) ecs: &'a Ecs,
    pub(super) table: &'a Table,
    pub(super) col_indices: &'a [usize],
    pub(super) fields: &'a [Access],
    pub(super) singletons: &'a [Access],
}

impl<'a> TableIter<'a> {
    #[inline(always)]
    pub fn num_rows(&self) -> usize {
        self.table.num_rows()
    }

    /// The entity ids for this table.
    #[inline(always)]
    pub fn ids(&self) -> &'a [Id] {
        self.table.ids()
    }

    #[inline]
    pub fn column<T: ReadOnlyFetch>(&self, idx: usize) -> T::ColumnSlice<'_> {
        crate::validate::check_access(T::ACCESS_TYPE, &self.fields[idx]);
        let idx = T::resolve(self.table, self.col_indices[idx]);
        let ptr = T::column_ptr(self.table, idx);
        let len = self.num_rows();
        unsafe { T::column_slice(ptr, len) }
    }

    #[inline]
    pub fn column_mut<T: ComponentFetch>(&mut self, idx: usize) -> T::ColumnSlice<'_> {
        crate::validate::check_access(T::ACCESS_TYPE, &self.fields[idx]);
        let idx = T::resolve(self.table, self.col_indices[idx]);
        let ptr = T::column_ptr(self.table, idx);
        let len = self.num_rows();
        unsafe { T::column_slice(ptr, len) }
    }

    /// Bulk slices for the whole validated row set. `&mut self` consumes the
    /// iterator's mutable capability once: the set's pairwise disjointness was
    /// proven by query validation, so handing out all slices together is sound,
    /// and you cannot take a second conflicting set.
    #[inline]
    pub fn columns<C: Columns>(&mut self) -> C::Slices<'_> {
        crate::validate::check_row(C::ACCESSES, self.fields);
        unsafe { C::slices(C::pointers(self), self.num_rows()) }
    }

    /// Get a slice for field at `index`.
    #[inline]
    pub fn resource<T: ComponentFetch>(&self, idx: usize) -> T::Get<'_> {
        crate::validate::check_access(T::ACCESS_TYPE, &self.singletons[idx]);
        T::resource(self.ecs, self.singletons[idx].id).unwrap()
    }

    pub fn each_row<T: Columns + 'a>(&'a self, mut f: impl FnMut(T::Row<'a>)) {
        crate::validate::check_row(T::ACCESSES, self.fields);
        let cols = T::pointers(self);
        for row in 0..self.num_rows() {
            f(unsafe { T::row(cols, row) })
        }
    }
}

pub trait Columns: Sized {
    type Pointers: Copy;
    type Slices<'a>;
    type Row<'c>;
    const ACCESSES: &'static [AccessType];

    fn pointers(iter: &TableIter) -> Self::Pointers;
    /// # Safety
    /// Validation proved these accesses pairwise disjoint; caller holds the
    /// unique mutable capability for the table (TableIter taken by &mut).
    unsafe fn slices<'a>(ptrs: Self::Pointers, count: usize) -> Self::Slices<'a>;
    unsafe fn row<'c>(column: Self::Pointers, row: usize) -> Self::Row<'c>;
}

impl<T: ComponentFetch> Columns for T {
    type Pointers = T::ColumnPtr;
    type Slices<'a> = T::ColumnSlice<'a>;
    type Row<'c> = T::Get<'c>;
    const ACCESSES: &'static [AccessType] = &[T::ACCESS_TYPE];

    fn pointers(iter: &TableIter) -> Self::Pointers {
        T::column_ptr(iter.table, T::resolve(iter.table, iter.col_indices[0]))
    }

    #[inline(always)]
    unsafe fn slices<'t>(ptrs: Self::Pointers, count: usize) -> Self::Slices<'t> {
        unsafe { T::column_slice(ptrs, count) }
    }

    unsafe fn row<'c>(column: Self::Pointers, row: usize) -> Self::Row<'c> {
        unsafe { T::row(column, row) }
    }
}

macro_rules! impl_tuple_row {
    ($($T:ident),+ $(,)?) => {
        impl<$($T: ComponentFetch),+> Columns for ($($T,)+) {
            type Row<'c> = ($($T::Get<'c>,)+);
            type Pointers = ($($T::ColumnPtr,)+);
            type Slices<'a> = ($($T::ColumnSlice<'a>,)+);
            const ACCESSES: &'static [AccessType] = &[$($T::ACCESS_TYPE),+];

            #[inline(always)]
            fn pointers(iter: &TableIter) -> Self::Pointers {
                let mut i = 0usize;
                ($(
                    #[allow(unused_assignments)]
                    {
                        let idx = $T::resolve(iter.table, iter.col_indices[i]);
                        let col = $T::column_ptr(iter.table, idx);
                        i += 1;
                        col
                    },
                )+)
            }


            #[inline(always)]
            unsafe fn slices<'a>(cols: Self::Pointers, count: usize) -> Self::Slices<'a> {
                #[allow(non_snake_case)]
                let ($($T,)+) = cols;
                unsafe { ($($T::column_slice($T, count),)+) }
            }

            #[inline(always)]
            unsafe fn row<'c>(cols: Self::Pointers, row: usize) -> Self::Row<'c> {
                #[allow(non_snake_case)]
                let ($($T,)+) = cols;
                unsafe { ($($T::row($T, row),)+) }
            }
        }
    };
}

impl_tuple_row!(T0, T1);
impl_tuple_row!(T0, T1, T2);
impl_tuple_row!(T0, T1, T2, T3);
impl_tuple_row!(T0, T1, T2, T3, T4);
impl_tuple_row!(T0, T1, T2, T3, T4, T5);
impl_tuple_row!(T0, T1, T2, T3, T4, T5, T6);
impl_tuple_row!(T0, T1, T2, T3, T4, T5, T6, T7);
impl_tuple_row!(T0, T1, T2, T3, T4, T5, T6, T7, T8);
impl_tuple_row!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_tuple_row!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_tuple_row!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_tuple_row!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
