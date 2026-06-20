use std::ptr::NonNull;

use crate::{
    Ecs, Error, Id, access::AccessType, component, error::EcsResult, query::fetch::private::SealedReadOnlyFetch,
    storage::table::Table,
};

mod private {
    pub trait SealedFetch {}
    pub trait SealedReadOnlyFetch {}
}

impl<T: 'static> private::SealedFetch for &T {}
impl<T: 'static> private::SealedFetch for &mut T {}
impl<T: 'static> private::SealedFetch for Option<&T> {}
impl<T: 'static> private::SealedFetch for Option<&mut T> {}
impl<T: 'static> private::SealedReadOnlyFetch for &T {}
impl<T: 'static> private::SealedReadOnlyFetch for Option<&T> {}

pub trait ComponentFetch: Sized + private::SealedFetch {
    type RemoveRef: 'static;
    type Get<'a>;
    type ColumnPtr: Copy;
    type ColumnSlice<'a>;
    type Index: Copy;
    const ACCESS_TYPE: AccessType;

    /// Resolve this fetch's column index against a table. Called once per table.
    fn resolve(table: &Table, raw: usize) -> Self::Index;

    /// Get a column from the table iterator.
    fn column_ptr(table: &Table, index: Self::Index) -> Self::ColumnPtr;

    unsafe fn column_slice<'t>(ptr: Self::ColumnPtr, len: usize) -> Self::ColumnSlice<'t>;

    /// # Safety
    /// Caller guarantees this row does not exceed the bounds of the column,
    /// and does not alias another live borrow of the same component
    unsafe fn row<'c>(col_ptr: Self::ColumnPtr, row: usize) -> Self::Get<'c>;

    fn resource<'w>(ecs: &'w Ecs, id: Id) -> EcsResult<Self::Get<'w>>;
}

impl<T: 'static> ComponentFetch for &T {
    type RemoveRef = T;
    type Get<'a> = &'a T;
    type ColumnPtr = NonNull<T>;
    type ColumnSlice<'a> = &'a [T];
    type Index = usize; // query match guarantees presence
    const ACCESS_TYPE: AccessType = AccessType::Read;

    #[inline(always)]
    fn resolve(_table: &Table, raw: usize) -> usize {
        raw
    }

    #[inline]
    unsafe fn row<'c>(col_ptr: NonNull<T>, row: usize) -> Self::Get<'c> {
        unsafe { col_ptr.add(row).as_ref() }
    }

    fn column_ptr(table: &Table, index: Self::Index) -> Self::ColumnPtr {
        table.column_ptr(index)
    }

    unsafe fn column_slice<'t>(ptr: Self::ColumnPtr, len: usize) -> Self::ColumnSlice<'t> {
        unsafe { core::slice::from_raw_parts(ptr.as_ptr(), len) }
    }

    fn resource<'w>(ecs: &'w Ecs, id: Id) -> EcsResult<Self::Get<'w>> {
        // SAFETY: ACCESS is Read; validation proved this resource was
        // declared Read or Write, so no conflicting &mut exists for 'w.
        unsafe { component::resource(ecs, id)?.ok_or(Error::MissingResource { id }) }
    }
}

impl<T: 'static> ComponentFetch for &mut T {
    type RemoveRef = T;
    type Get<'a> = &'a mut T;
    type Index = usize; // query match guarantees presence
    type ColumnPtr = NonNull<T>;
    type ColumnSlice<'a> = &'a mut [T];
    const ACCESS_TYPE: AccessType = AccessType::Write;

    #[inline(always)]
    fn resolve(_table: &Table, raw: usize) -> usize {
        raw
    }

    #[inline]
    unsafe fn row<'c>(col_ptr: Self::ColumnPtr, row: usize) -> Self::Get<'c> {
        unsafe { col_ptr.add(row).as_mut() }
    }

    #[inline]
    fn column_ptr<'t>(table: &'t Table, index: Self::Index) -> Self::ColumnPtr {
        table.column_ptr(index)
    }

    unsafe fn column_slice<'t>(ptr: Self::ColumnPtr, len: usize) -> Self::ColumnSlice<'t> {
        unsafe { core::slice::from_raw_parts_mut(ptr.as_ptr(), len) }
    }

    fn resource<'w>(ecs: &'w Ecs, id: Id) -> EcsResult<Self::Get<'w>> {
        // SAFETY: ACCESS is Read; validation proved this resource was
        // declared Read or Write, so no conflicting &mut exists for 'w.
        unsafe { component::resource_mut(ecs, id)?.ok_or(Error::MissingResource { id }) }
    }
}

impl<T: 'static> ComponentFetch for Option<&T> {
    type RemoveRef = T;
    type Get<'a> = Option<&'a T>;
    type Index = Option<usize>;
    type ColumnPtr = Option<NonNull<T>>;
    type ColumnSlice<'a> = Option<&'a [T]>;
    const ACCESS_TYPE: AccessType = AccessType::Read;

    #[inline]
    fn resolve(_table: &Table, raw: usize) -> Option<usize> {
        (raw != usize::MAX).then_some(raw)
    }

    unsafe fn row<'c>(col_ptr: Self::ColumnPtr, row: usize) -> Self::Get<'c> {
        col_ptr.map(|col| unsafe { col.add(row).as_ref() })
    }

    fn column_ptr<'t>(table: &'t Table, index: Self::Index) -> Self::ColumnPtr {
        index.map(|i| table.column_ptr(i))
    }

    unsafe fn column_slice<'t>(ptr: Self::ColumnPtr, len: usize) -> Self::ColumnSlice<'t> {
        ptr.map(|c| unsafe { core::slice::from_raw_parts(c.as_ptr(), len) })
    }

    fn resource<'w>(ecs: &'w Ecs, id: Id) -> EcsResult<Self::Get<'w>> {
        // SAFETY: ACCESS is Read; validation proved this resource was
        // declared Read or Write, so no conflicting &mut exists for 'w.
        unsafe { component::resource(ecs, id).map_err(Into::into) }
    }
}

impl<T: 'static> ComponentFetch for Option<&mut T> {
    type RemoveRef = T;
    type Get<'a> = Option<&'a mut T>;
    type Index = Option<usize>;
    type ColumnPtr = Option<NonNull<T>>;
    type ColumnSlice<'a> = Option<&'a [T]>;
    const ACCESS_TYPE: AccessType = AccessType::Write;

    #[inline]
    fn resolve(_table: &Table, raw: usize) -> Option<usize> {
        // raw is the matcher's resolved index, or sentinel for absent.
        // Cleaner: resolve directly from the col_map. See note below.
        (raw != usize::MAX).then_some(raw)
    }

    unsafe fn row<'c>(col_ptr: Self::ColumnPtr, row: usize) -> Self::Get<'c> {
        col_ptr.map(|col| unsafe { col.add(row).as_mut() })
    }

    fn column_ptr<'t>(table: &'t Table, index: Self::Index) -> Self::ColumnPtr {
        index.map(|i| table.column_ptr(i))
    }

    unsafe fn column_slice<'t>(ptr: Self::ColumnPtr, len: usize) -> Self::ColumnSlice<'t> {
        ptr.map(|c| unsafe { core::slice::from_raw_parts(c.as_ptr(), len) })
    }

    fn resource<'w>(ecs: &'w Ecs, id: Id) -> EcsResult<Self::Get<'w>> {
        // SAFETY: ACCESS is Read; validation proved this resource was
        // declared Read or Write, so no conflicting &mut exists for 'w.
        unsafe { component::resource_mut(ecs, id).map_err(Into::into) }
    }
}

pub trait ReadOnlyFetch: ComponentFetch + SealedReadOnlyFetch {}
impl<T: 'static> ReadOnlyFetch for &T {}
impl<T: 'static> ReadOnlyFetch for Option<&T> {}

/*pub trait GetMulti: Sized + private::SealedGetMulti {
    type Output<'a>;
    type Query;
    fn get(ecs: &mut Ecs, id: Id, query: Self::Query) -> EcsResult<Self::Output<'_>>;
}

macro_rules! count {
    () => { 0 };
    ($head:tt $($rest:tt)*) => { 1 + count!($($rest)*) };
}

macro_rules! impl_tuple_params {
    ($($T:ident),+) => {
        impl<$($T: ComponentFetch),*> private::SealedGetMulti for ($($T),+) {}
        impl<$($T: ComponentFetch),*> GetMulti for ($($T,)+) {
            type Output<'a> = ($($T::Get<'a>,)+);
            type Query = [Id; { count!($($T)+) }];

            fn get(ecs: &mut Ecs, id: Id, query: Self::Query) -> EcsResult<Self::Output<'_>> {
                let r = ecs.ids.get(id)?;

                #[allow(non_snake_case)]
                let [$($T,)+] = query;

                //todo!("validation");

                // SAFETY: check_multi_get proved no aliasing among the tuple's
                // accesses; &mut Ecs proves no external aliasing. Each fetch is
                // therefore the unique live borrow of its component.
                let ecs: &Ecs = ecs;
                Ok(unsafe { ($($T::get(ecs, id, r, $T )?,)*) })
            }
        }
    }
}

impl_tuple_params!(T0, T1);
impl_tuple_params!(T0, T1, T2);
impl_tuple_params!(T0, T1, T2, T3);
impl_tuple_params!(T0, T1, T2, T3, T4);
impl_tuple_params!(T0, T1, T2, T3, T4, T5);
impl_tuple_params!(T0, T1, T2, T3, T4, T5, T6);
impl_tuple_params!(T0, T1, T2, T3, T4, T5, T6, T7);
impl_tuple_params!(T0, T1, T2, T3, T4, T5, T6, T7, T8);
impl_tuple_params!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_tuple_params!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_tuple_params!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_tuple_params!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);*/
