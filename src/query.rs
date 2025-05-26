use crate::{
    component::Component, error::EcsResult, id::Id, pointer::PtrMut, storage::Storage,
    table_index::TableId, world::World,
};
use xecs_macros::{all_tuples, params};

pub trait ParamItem {
    type BaseType: Component;
    type ActualType<'a>;
    const IS_OPTIONAL: bool;
    const IS_IMMUTABLE: bool;
    fn to_param<'a>(ptr: Option<PtrMut<'a>>) -> Self::ActualType<'a>;
}

impl<T: Component> ParamItem for &T {
    type BaseType = T;
    type ActualType<'a> = &'a T;
    const IS_OPTIONAL: bool = false;
    const IS_IMMUTABLE: bool = true;

    fn to_param<'a>(ptr: Option<PtrMut<'a>>) -> Self::ActualType<'a> {
        // TODO: SAFETY
        unsafe { ptr.unwrap_unchecked().as_ref() }
    }
}

impl<T: Component> ParamItem for &mut T {
    type BaseType = T;
    type ActualType<'a> = &'a mut T;
    const IS_OPTIONAL: bool = false;
    const IS_IMMUTABLE: bool = false;

    fn to_param<'a>(ptr: Option<PtrMut<'a>>) -> Self::ActualType<'a> {
        // TODO: SAFETY
        unsafe { ptr.unwrap_unchecked().as_mut::<T>() }
    }
}

impl<T: Component> ParamItem for Option<&T> {
    type BaseType = T;
    type ActualType<'a> = Option<&'a T>;
    const IS_OPTIONAL: bool = true;
    const IS_IMMUTABLE: bool = true;

    fn to_param<'a>(ptr: Option<PtrMut<'a>>) -> Self::ActualType<'a> {
        ptr.map(|p| unsafe { p.as_ref() })
    }
}

impl<T: Component> ParamItem for Option<&mut T> {
    type BaseType = T;
    type ActualType<'a> = Option<&'a mut T>;
    const IS_OPTIONAL: bool = true;
    const IS_IMMUTABLE: bool = false;

    fn to_param<'a>(ptr: Option<PtrMut<'a>>) -> Self::ActualType<'a> {
        ptr.map(|p| unsafe { p.as_mut::<T>() })
    }
}

fn get_ptr_opt<C: Component>(
    world: &mut World,
    id: Id,
    table_id: TableId,
    row: usize,
) -> Option<PtrMut> {
    let comp = world.id_t::<C>()?;
    let ci = world.components.get_mut(comp)?;

    // SAFETY: Valid id must have valid table and row.
    match &mut ci.storage {
        Storage::SparseTag(_) => None,
        Storage::SparseData(set) => set.get_ptr_mut(id),
        Storage::Tables(_) => unsafe { world.table_index[table_id].get_ptr_mut(row, comp) },
    }
}

pub trait Params: Sized {
    type ParamType<'a>;
    const ALL_IMMUTABLE: bool;
    fn create<'a>(world: &'a mut World, id: Id) -> EcsResult<Self::ParamType<'a>>;
}

impl<T: ParamItem> Params for T {
    type ParamType<'a> = T::ActualType<'a>;
    const ALL_IMMUTABLE: bool = T::IS_IMMUTABLE;

    fn create<'a>(world: &'a mut World, id: Id) -> EcsResult<Self::ParamType<'a>> {
        let (table_id, row) = world.id_index.get_location(id)?;

        let ptr = if T::IS_OPTIONAL {
            get_ptr_opt::<T::BaseType>(world, id, table_id, row)
        } else {
            None
        };

        Ok(T::to_param(ptr))
    }
}

macro_rules! tuple_count {
    () => { 0 };
    ($head:ident) => { 1 };
    ($head:ident, $($tail:ident),*) => { 1 + tuple_count!($($tail),*) };
}

macro_rules! impl_tuple_params {
    ($($t:ident),*) => {
        impl<$($t: ParamItem),*> Params for ($($t,)*) {
            type ParamType<'a> = ($($t::ActualType<'a>,)*);
            const ALL_IMMUTABLE: bool = { $($t::IS_IMMUTABLE &&)* true };

            fn create<'a>(world: &'a mut World, id: Id) -> EcsResult<Self::ParamType<'a>> {
                todo!()
            }
        }
    }
}

all_tuples!(impl_tuple_params, 1, 2);

struct Position;
struct Velocity;
struct Mass;
struct Health;

fn test<T>() {
    println!("{}", std::any::type_name::<T>())
}

fn call_test<T>() {
    test::<params!(mut T?)>();
}

#[test]
fn lets_test() {
    call_test::<std::collections::HashMap<Vec<usize>, std::collections::BTreeMap<u8, String>>>();
}
