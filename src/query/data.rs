use std::ptr::NonNull;

use crate::{
    Component, component,
    id::Id,
    query::access::{AccessError, AccessSet},
    table::Table,
};

/// Data a query fetches for each matched row.
///
/// # Safety
/// - `access` reports every component the term reads or writes.
/// - `resolve` returns `Some` only for tables holding everything the term
///   needs, and the state it returns is valid for that table.
/// - `fetch_row` produces references only to data `access` reports, with
///   matching mutability.
pub unsafe trait QueryData {
    type Value<'a>;
    /// Resolved once when a table is matched, such as column indices.
    type State: Copy;
    /// Built from `State` at the start of each run, such as column pointers.
    type Fetch: Copy;

    fn access(access: &mut AccessSet) -> Result<(), AccessError>;

    fn resolve(table: &Table) -> Option<Self::State>;

    /// # Safety
    /// `state` was resolved for `table`.
    unsafe fn fetch(state: Self::State, table: &Table) -> Self::Fetch;

    /// # Safety
    /// - `fetch` was built for a table that is unchanged and has more than
    ///   `row` rows.
    /// - The caller holds the access `access` reports for `'a`.
    unsafe fn fetch_row<'a>(fetch: Self::Fetch, row: usize) -> Self::Value<'a>;
}

/// Query data that never writes.
///
/// # Safety
/// `access` reports no writes.
pub unsafe trait ReadOnlyData: QueryData {}

/// Query data that can be handed out as slices over a whole table.
///
/// # Safety
/// `slice` produces references only to data `access` reports, with matching
/// mutability.
pub unsafe trait SliceData: QueryData {
    type Slice<'a>;

    /// # Safety
    /// - `fetch` was built for a table that is unchanged and has exactly
    ///   `len` rows, with `len > 0`.
    /// - The caller holds the access `access` reports for `'a`.
    unsafe fn slice<'a>(fetch: Self::Fetch, len: usize) -> Self::Slice<'a>;
}

/// Column index of `K` in `table`.
#[inline]
fn column_of<K: Component>(table: &Table) -> Option<u32> {
    table.sig.find_id(&component::id::<K>()).map(|col| col as u32)
}

// SAFETY: reads `K`'s column, which `resolve` found in the table.
unsafe impl<K: Component> QueryData for &K {
    type Value<'a> = &'a K::Data;
    type State = u32;
    type Fetch = NonNull<K::Data>;

    fn access(access: &mut AccessSet) -> Result<(), AccessError> {
        access.read(component::id::<K>())
    }

    #[inline]
    fn resolve(table: &Table) -> Option<Self::State> {
        column_of::<K>(table)
    }

    #[inline(always)]
    unsafe fn fetch(col: Self::State, table: &Table) -> Self::Fetch {
        table.column_ptr(col).cast()
    }

    #[inline(always)]
    unsafe fn fetch_row<'a>(fetch: Self::Fetch, row: usize) -> Self::Value<'a> {
        // SAFETY: `row` is in bounds, and the column holds `K::Value`.
        unsafe { fetch.add(row).as_ref() }
    }
}

// SAFETY: only reads.
unsafe impl<K: Component> ReadOnlyData for &K {}

// SAFETY: a slice over `K`'s column, read-only.
unsafe impl<K: Component> SliceData for &K {
    type Slice<'a> = &'a [K::Data];

    #[inline(always)]
    unsafe fn slice<'a>(fetch: Self::Fetch, len: usize) -> Self::Slice<'a> {
        // SAFETY: the column holds `len` initialized values of `K::Value`.
        unsafe { std::slice::from_raw_parts(fetch.as_ptr(), len) }
    }
}

// SAFETY: writes `K`'s column, which `resolve` found in the table.
unsafe impl<K: Component> QueryData for &mut K {
    type Value<'a> = &'a mut K::Data;
    type State = u32;
    type Fetch = NonNull<K::Data>;

    fn access(access: &mut AccessSet) -> Result<(), AccessError> {
        access.write(component::id::<K>())
    }

    #[inline]
    fn resolve(table: &Table) -> Option<Self::State> {
        column_of::<K>(table)
    }

    #[inline(always)]
    unsafe fn fetch(col: Self::State, table: &Table) -> Self::Fetch {
        table.column_ptr(col).cast()
    }

    #[inline(always)]
    unsafe fn fetch_row<'a>(fetch: Self::Fetch, row: usize) -> Self::Value<'a> {
        // SAFETY: `row` is in bounds, the column holds `K::Value`, and no
        // other term accesses `K`.
        unsafe { fetch.add(row).as_mut() }
    }
}

// SAFETY: a mutable slice over `K`'s column, which no other term accesses.
unsafe impl<K: Component> SliceData for &mut K {
    type Slice<'a> = &'a mut [K::Data];

    #[inline(always)]
    unsafe fn slice<'a>(fetch: Self::Fetch, len: usize) -> Self::Slice<'a> {
        // SAFETY: the column holds `len` initialized values of `K::Value`.
        unsafe { std::slice::from_raw_parts_mut(fetch.as_ptr(), len) }
    }
}

// SAFETY: reads the table's id array, which every table has.
unsafe impl QueryData for Id {
    type Value<'a> = Id;
    type State = ();
    type Fetch = NonNull<Id>;

    fn access(_: &mut AccessSet) -> Result<(), AccessError> {
        Ok(())
    }

    #[inline]
    fn resolve(_: &Table) -> Option<Self::State> {
        Some(())
    }

    #[inline(always)]
    unsafe fn fetch(_: Self::State, table: &Table) -> Self::Fetch {
        NonNull::from(table.ids()).cast()
    }

    #[inline(always)]
    unsafe fn fetch_row<'a>(fetch: Self::Fetch, row: usize) -> Self::Value<'a> {
        // SAFETY: `row` is in bounds of the id array.
        unsafe { fetch.add(row).read() }
    }
}

// SAFETY: only reads.
unsafe impl ReadOnlyData for Id {}

// SAFETY: a slice over the table's id array, read-only.
unsafe impl SliceData for Id {
    type Slice<'a> = &'a [Id];

    #[inline(always)]
    unsafe fn slice<'a>(fetch: Self::Fetch, len: usize) -> Self::Slice<'a> {
        // SAFETY: the table has `len` ids.
        unsafe { std::slice::from_raw_parts(fetch.as_ptr(), len) }
    }
}

// SAFETY: forwards to `T` when `T` resolves, and yields `None` otherwise.
unsafe impl<T: QueryData> QueryData for Option<T> {
    type Value<'a> = Option<T::Value<'a>>;
    type State = Option<T::State>;
    type Fetch = Option<T::Fetch>;

    fn access(access: &mut AccessSet) -> Result<(), AccessError> {
        T::access(access)
    }

    #[inline]
    fn resolve(table: &Table) -> Option<Self::State> {
        Some(T::resolve(table))
    }

    #[inline(always)]
    unsafe fn fetch(state: Self::State, table: &Table) -> Self::Fetch {
        // SAFETY: an inner state was resolved for `table`.
        state.map(|s| unsafe { T::fetch(s, table) })
    }

    #[inline(always)]
    unsafe fn fetch_row<'a>(fetch: Self::Fetch, row: usize) -> Self::Value<'a> {
        // SAFETY: forwarded from the caller.
        fetch.map(|f| unsafe { T::fetch_row(f, row) })
    }
}

// SAFETY: only reads when `T` only reads.
unsafe impl<T: ReadOnlyData> ReadOnlyData for Option<T> {}

// SAFETY: forwards to `T` when `T` resolved.
unsafe impl<T: SliceData> SliceData for Option<T> {
    type Slice<'a> = Option<T::Slice<'a>>;

    #[inline(always)]
    unsafe fn slice<'a>(fetch: Self::Fetch, len: usize) -> Self::Slice<'a> {
        // SAFETY: forwarded from the caller.
        fetch.map(|f| unsafe { T::slice(f, len) })
    }
}

macro_rules! impl_tuple {
    ($($T:ident),+) => {
        // SAFETY: every member upholds the contract, and `access` checks all
        // members together, so no two members alias.
        unsafe impl<$($T: QueryData),+> QueryData for ($($T,)+) {
            type Value<'a> = ($(<$T as QueryData>::Value<'a>,)+);
            type State = ($($T::State,)+);
            type Fetch = ($($T::Fetch,)+);

            fn access(access: &mut AccessSet) -> Result<(), AccessError> {
                $($T::access(access)?;)+
                Ok(())
            }

            #[inline]
            fn resolve(table: &Table) -> Option<Self::State> {
                Some(($($T::resolve(table)?,)+))
            }

            #[inline(always)]
            unsafe fn fetch(state: Self::State, table: &Table) -> Self::Fetch {
                #[allow(non_snake_case)]
                let ($($T,)+) = state;
                // SAFETY: each member's state was resolved for `table`.
                unsafe { ($($T::fetch($T, table),)+) }
            }

            #[inline(always)]
            unsafe fn fetch_row<'a>(fetch: Self::Fetch, row: usize) -> Self::Value<'a> {
                #[allow(non_snake_case)]
                let ($($T,)+) = fetch;
                // SAFETY: forwarded from the caller.
                unsafe { ($($T::fetch_row($T, row),)+) }
            }
        }

        // SAFETY: only reads when every member only reads.
        unsafe impl<$($T: ReadOnlyData),+> ReadOnlyData for ($($T,)+) {}

        // SAFETY: every member upholds the contract.
        unsafe impl<$($T: SliceData),+> SliceData for ($($T,)+) {
            type Slice<'a> = ($(<$T as SliceData>::Slice<'a>,)+);

            #[inline(always)]
            unsafe fn slice<'a>(fetch: Self::Fetch, len: usize) -> Self::Slice<'a> {
                #[allow(non_snake_case)]
                let ($($T,)+) = fetch;
                // SAFETY: forwarded from the caller.
                unsafe { ($($T::slice($T, len),)+) }
            }
        }
    };
}

impl_tuple!(T0);
impl_tuple!(T0, T1);
impl_tuple!(T0, T1, T2);
impl_tuple!(T0, T1, T2, T3);
impl_tuple!(T0, T1, T2, T3, T4);
impl_tuple!(T0, T1, T2, T3, T4, T5);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
