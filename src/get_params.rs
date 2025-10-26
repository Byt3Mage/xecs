use crate::{
    error::{GetError, GetResult},
    id::{Id, manager::IdLocation},
    type_traits::{DataComponent, TypedId},
    world::World,
};
use private::Sealed;
use xecs_macros::all_tuples;

mod private {
    pub trait Sealed {}
}

pub trait GetParam: Sealed {
    type Data: DataComponent;
    type Output<'a>;
    const IS_IMMUTABLE: bool;
    fn make(world: &World, id: Id, loc: IdLocation) -> GetResult<Self::Output<'_>>;
}

impl<T: GetParam> private::Sealed for T {}

impl<T> GetParam for &T
where
    T: TypedId + DataComponent,
    <T as TypedId>::Data: DataComponent,
{
    type Data = <T as TypedId>::Data;
    type Output<'a> = &'a Self::Data;
    const IS_IMMUTABLE: bool = true;

    fn make(world: &World, id: Id, loc: IdLocation) -> GetResult<Self::Output<'_>> {
        let comp = T::id(world)?;
        let comp_info = match world.components.get(comp) {
            Some(ci) => ci,
            None => return Err(GetError::IdNotComponent(comp)),
        };

        match &comp_info.storage {
            crate::storage::Storage::SparseTag(_) => return Err(GetError::IdNotComponent(comp)),
            crate::storage::Storage::SparseData(set) => unsafe { set.get::<Self::Data>(id) },
            crate::storage::Storage::Tables(_) => unsafe {
                world.table_index[loc.table].get::<Self::Data>(comp, loc.row)
            },
        }
        .ok_or(GetError::MissingComponent(comp))
    }
}

impl<T> GetParam for &mut T
where
    T: TypedId + DataComponent,
    <T as TypedId>::Data: DataComponent,
{
    type Data = <T as TypedId>::Data;
    type Output<'a> = &'a mut Self::Data;
    const IS_IMMUTABLE: bool = false;

    fn make(world: &World, id: Id, loc: IdLocation) -> GetResult<Self::Output<'_>> {
        // SAFETY: We have checked component ids to prevent aliasing.
        let world = todo!();
        let comp = T::id(world)?;
        let comp_info = match world.components.get_mut(comp) {
            Some(ci) => ci,
            None => return Err(GetError::IdNotComponent(comp)),
        };

        match &mut comp_info.storage {
            crate::storage::Storage::SparseTag(_) => return Err(GetError::IdNotComponent(comp)),
            crate::storage::Storage::SparseData(set) => unsafe { set.get_mut::<Self::Data>(id) },
            crate::storage::Storage::Tables(_) => unsafe {
                world.table_index[loc.table].get_mut::<Self::Data>(loc.row, comp)
            },
        }
        .ok_or(GetError::MissingComponent(comp))
    }
}

impl<T> GetParam for Option<&T>
where
    T: TypedId + DataComponent,
    <T as TypedId>::Data: DataComponent,
{
    type Data = <T as TypedId>::Data;
    type Output<'a> = Option<&'a Self::Data>;
    const IS_IMMUTABLE: bool = true;

    fn make(world: &World, id: Id, loc: IdLocation) -> GetResult<Self::Output<'_>> {
        // SAFETY: We have checked component ids to prevent aliasing.
        let world = todo!();
        let Ok(comp) = T::id(world) else {
            return Ok(None);
        };
        let Some(comp_info) = world.components.get(comp) else {
            return Ok(None);
        };

        Ok(match &comp_info.storage {
            crate::storage::Storage::SparseTag(_) => return Ok(None),
            crate::storage::Storage::SparseData(set) => unsafe { set.get::<Self::Data>(id) },
            crate::storage::Storage::Tables(_) => unsafe {
                world.table_index[loc.table].get::<Self::Data>(comp, loc.row)
            },
        })
    }
}

impl<T: TypedId + DataComponent> GetParam for Option<&mut T>
where
    T: TypedId + DataComponent,
    <T as TypedId>::Data: DataComponent,
{
    type Data = <T as TypedId>::Data;
    type Output<'a> = Option<&'a mut Self::Data>;
    const IS_IMMUTABLE: bool = false;

    fn make(world: &World, id: Id, loc: IdLocation) -> GetResult<Self::Output<'_>> {
        // SAFETY: We have checked component ids to prevent aliasing.
        let world = todo!();
        let Ok(comp) = T::id(world) else {
            return Ok(None);
        };
        let Some(comp_info) = world.components.get_mut(comp) else {
            return Ok(None);
        };

        Ok(match &mut comp_info.storage {
            crate::storage::Storage::SparseTag(_) => return Ok(None),
            crate::storage::Storage::SparseData(set) => unsafe { set.get_mut::<Self::Data>(id) },
            crate::storage::Storage::Tables(_) => unsafe {
                world.table_index[loc.table].get_mut::<Self::Data>(loc.row, comp)
            },
        })
    }
}

pub trait Params: Sized + private::Sealed {
    type ParamsType<'a>;
    const ALL_IMMUTABLE: bool;
    fn create(world: &World, id: Id) -> GetResult<Self::ParamsType<'_>>;
}

impl<T: GetParam> Params for T {
    type ParamsType<'a> = T::Output<'a>;
    const ALL_IMMUTABLE: bool = T::IS_IMMUTABLE;

    fn create(world: &World, id: Id) -> GetResult<Self::ParamsType<'_>> {
        let id_loc = world.id_manager.get_location(id)?;
        T::make(world, id, id_loc)
    }
}

macro_rules! impl_tuple_params {
    ($($t:ident),*) => {
        impl<$($t: GetParam),*> private::Sealed for ($($t,)*) {}
        impl<$($t: GetParam),*> Params for ($($t,)*) {
            type ParamsType<'a> = ($($t::Output<'a>,)*);
            const ALL_IMMUTABLE: bool = { $($t::IS_IMMUTABLE &&)* true };

            fn create(world: &World, id: Id) -> GetResult<Self::ParamsType<'_>> {
                let id_loc = world.id_manager.get_location(id)?;

                if !Self::ALL_IMMUTABLE {
                    panic!("mutable access not yet supported")
                }

                Ok(($($t::make(world, id, id_loc)?,)*))
            }
        }
    }
}

all_tuples!(impl_tuple_params, 1, 13);
