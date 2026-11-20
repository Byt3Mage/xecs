use std::marker::PhantomData;

use crate::{bundle::ColumnSet, table::Table};

/// A condition on a table's signature, checked once when the table is matched.
pub trait QueryFilter: 'static {
    fn matches(table: &Table) -> bool;
}

impl QueryFilter for () {
    #[inline]
    fn matches(_: &Table) -> bool {
        true
    }
}

/// Matches tables holding every component of `S`.
pub struct With<S: ColumnSet>(PhantomData<fn() -> S>);

impl<S: ColumnSet> QueryFilter for With<S> {
    #[inline]
    fn matches(table: &Table) -> bool {
        S::KEYS.iter().all(|k| table.sig.has_id(&k.id()))
    }
}

/// Matches tables holding no component of `S`.
pub struct Without<S: ColumnSet>(PhantomData<fn() -> S>);

impl<S: ColumnSet> QueryFilter for Without<S> {
    #[inline]
    fn matches(table: &Table) -> bool {
        !S::KEYS.iter().any(|k| table.sig.has_id(&k.id()))
    }
}

macro_rules! impl_tuple {
    ($($F:ident),+) => {
        impl<$($F: QueryFilter),+> QueryFilter for ($($F,)+) {
            #[inline]
            fn matches(table: &Table) -> bool {
                true $(&& $F::matches(table))+
            }
        }
    };
}

impl_tuple!(F0);
impl_tuple!(F0, F1);
impl_tuple!(F0, F1, F2);
impl_tuple!(F0, F1, F2, F3);
impl_tuple!(F0, F1, F2, F3, F4);
impl_tuple!(F0, F1, F2, F3, F4, F5);
impl_tuple!(F0, F1, F2, F3, F4, F5, F6);
impl_tuple!(F0, F1, F2, F3, F4, F5, F6, F7);
