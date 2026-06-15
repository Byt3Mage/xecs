use crate::{
    Ecs,
    component::{self, ComponentAccess},
    id::Id,
    query::{Access, AccessType},
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

    /// Get a slice for field at `index`.
    #[inline]
    pub fn column<T: Field>(&self, idx: usize) -> T::Column<'_> {
        crate::validate::check_access(T::ACCESS, &self.fields[idx]);
        T::column(self, idx)
    }

    /// Get a slice for field at `index`.
    #[inline]
    pub fn resource<T: Field>(&self, idx: usize) -> T::Row<'_> {
        crate::validate::check_access(T::ACCESS, &self.singletons[idx]);
        T::resource(self, idx)
    }

    pub fn each_row<T: Row + 'a>(&'a self, mut f: impl FnMut(T::Get<'a>)) {
        crate::validate::check_row(T::ACCESSES, self.fields);
        let mut cols = T::columns(self);
        (0..self.num_rows()).for_each(|i| f(unsafe { T::get(&mut cols, i) }));
    }
}

pub trait Field: ComponentAccess {
    type Row<'c>;
    type Column<'t>;

    unsafe fn row<'c>(column: &mut Self::Column<'c>, row: usize) -> Self::Row<'c>;
    fn column<'t>(iter: &'t TableIter<'t>, index: usize) -> Self::Column<'t>;
    fn resource<'t>(iter: &'t TableIter<'t>, index: usize) -> Self::Row<'t>;
}

impl<T: 'static> Field for &T {
    type Row<'c> = &'c T;
    type Column<'t> = &'t [T];

    unsafe fn row<'c>(column: &mut Self::Column<'c>, row: usize) -> Self::Row<'c> {
        // Reborrow to detach lifetime
        unsafe { &*(column.get_unchecked(row) as *const T) }
    }

    fn column<'t>(iter: &'t TableIter<'t>, index: usize) -> Self::Column<'t> {
        let col = iter.col_indices[index];
        // SAFETY: ACCESS is Read; validation proved this column was
        // declared Read or Write, so no conflicting &mut exists for 't.
        unsafe { iter.table.col_slice::<T>(col) }
    }

    fn resource<'t>(iter: &'t TableIter<'t>, index: usize) -> Self::Row<'t> {
        let col = iter.singletons[index];
        // SAFETY: ACCESS is Read; validation proved this resource was
        // declared Read or Write, so no conflicting &mut exists for 't.
        unsafe { component::resource(iter.ecs, col.id).unwrap() }
    }
}

impl<T: 'static> Field for &mut T {
    type Row<'c> = &'c mut T;
    type Column<'t> = &'t mut [T];

    unsafe fn row<'c>(column: &mut Self::Column<'c>, row: usize) -> Self::Row<'c> {
        // Reborrow to detach lifetime
        unsafe { &mut *(column.get_unchecked_mut(row) as *mut T) }
    }

    fn column<'t>(iter: &'t TableIter<'t>, index: usize) -> Self::Column<'t> {
        let col = iter.col_indices[index];
        // SAFETY: ACCESS is Write; validation proved this column was declared
        // Write and is internally unique (AccessList) and disjoint from any
        // combined query (check_disjoint), so this &mut is the only borrow for 't.
        unsafe { iter.table.col_slice_mut::<T>(col) }
    }

    fn resource<'t>(iter: &'t TableIter<'t>, index: usize) -> Self::Row<'t> {
        let col = iter.singletons[index];
        // SAFETY: ACCESS is Write; validation proved this resource was declared
        // Write and is internally unique (AccessList) and disjoint from any
        // combined query (check_disjoint), so this &mut is the only borrow for 't.
        unsafe { component::resource_mut(iter.ecs, col.id).unwrap() }
    }
}

pub trait Row: Sized {
    type Get<'c>;
    type Columns<'t>;
    const ACCESSES: &'static [AccessType];

    fn columns<'t>(iter: &'t TableIter<'t>) -> Self::Columns<'t>;
    unsafe fn get<'c>(column: &mut Self::Columns<'c>, row: usize) -> Self::Get<'c>;
}

impl<T: Field> Row for T {
    type Get<'c> = T::Row<'c>;
    type Columns<'t> = T::Column<'t>;
    const ACCESSES: &'static [AccessType] = &[T::ACCESS];

    fn columns<'t>(iter: &'t TableIter<'t>) -> Self::Columns<'t> {
        T::column(iter, 0)
    }

    unsafe fn get<'c>(column: &mut Self::Columns<'c>, row: usize) -> Self::Get<'c> {
        unsafe { T::row(column, row) }
    }
}

macro_rules! impl_tuple_row {
    ($($T:ident),+ $(,)?) => {
        impl<$($T: Field),+ > Row for ($($T,)+) {
            type Get<'c> = ($($T::Row<'c>,)+);
            type Columns<'t> = ($($T::Column<'t>,)+);
            const ACCESSES: &'static [AccessType] = &[$($T::ACCESS),+];

            #[inline(always)]
            fn columns<'t>(iter: &'t TableIter<'t>) -> Self::Columns<'t> {
                let mut i = 0usize;
                ($(
                    #[allow(unused_assignments)]
                    { let col = $T::column(iter, i); i += 1; col },
                )+)
            }

            unsafe fn get<'c>(cols: & mut Self::Columns<'c>, row: usize) -> Self::Get<'c> {
                #[allow(non_snake_case)]
                let ($($T,)+) = cols;
                unsafe { ($($T::row($T, row),)+) }
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
