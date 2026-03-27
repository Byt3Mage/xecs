use crate::{
    component::Component,
    error::GetError,
    id::{Entity, Id, entity_manager::EntityLocation},
    type_traits::{Data, TypedId},
    world::World,
};
use private::Sealed;

mod private {
    pub trait Sealed {}
}

pub trait GetParam: Sealed {
    type Data: Component<DataType = Data>;
    type Output<'a>;
    const IS_IMMUTABLE: bool;

    fn make(world: &World, e: Entity, loc: EntityLocation) -> Result<Self::Output<'_>, GetError>;
}

impl<T: GetParam> private::Sealed for T {}
impl<T: TypedId> GetParam for &T
where
    <T as TypedId>::Data: Component<DataType = Data>,
{
    type Data = <T as TypedId>::Data;
    type Output<'a> = &'a Self::Data;
    const IS_IMMUTABLE: bool = true;

    fn make(world: &World, e: Entity, loc: EntityLocation) -> Result<Self::Output<'_>, GetError> {
        let comp_id = T::id(world)?;

        let Some(ci) = comp_id.map_get(&world.components) else {
            return Err(GetError::IdNotComponent(Entity::NULL));
        };

        let res = unsafe {
            match &ci.storage {
                crate::storage::Storage::Tables(_) => {
                    world.tables[loc.table].get::<Self::Data>(&comp_id, loc.row)
                }
                crate::storage::Storage::SparseData(set) => set.get::<Self::Data>(e),
                crate::storage::Storage::SparseTag(_) => {
                    return Err(GetError::IdNotComponent(Entity::NULL));
                }
            }
        };

        res.ok_or(GetError::MissingComponent(Entity::NULL))
    }
}

impl<T: TypedId> GetParam for &mut T
where
    <T as TypedId>::Data: Component<DataType = Data>,
{
    type Data = <T as TypedId>::Data;
    type Output<'a> = &'a mut Self::Data;
    const IS_IMMUTABLE: bool = false;

    fn make(world: &World, id: Entity, loc: EntityLocation) -> Result<Self::Output<'_>, GetError> {
        todo!()
    }
}

impl<T: TypedId> GetParam for Option<&T>
where
    <T as TypedId>::Data: Component<DataType = Data>,
{
    type Data = <T as TypedId>::Data;
    type Output<'a> = Option<&'a Self::Data>;
    const IS_IMMUTABLE: bool = true;

    fn make(world: &World, id: Entity, loc: EntityLocation) -> Result<Self::Output<'_>, GetError> {
        todo!()
    }
}

impl<T: TypedId> GetParam for Option<&mut T>
where
    <T as TypedId>::Data: Component<DataType = Data>,
{
    type Data = <T as TypedId>::Data;
    type Output<'a> = Option<&'a mut Self::Data>;
    const IS_IMMUTABLE: bool = false;

    fn make(world: &World, id: Entity, loc: EntityLocation) -> Result<Self::Output<'_>, GetError> {
        todo!()
    }
}

pub trait Params: Sized + private::Sealed {
    type ParamsType<'a>;
    const ALL_IMMUTABLE: bool;
    fn create(world: &World, id: Entity) -> Result<Self::ParamsType<'_>, GetError>;
}

impl<T: GetParam> Params for T {
    type ParamsType<'a> = T::Output<'a>;
    const ALL_IMMUTABLE: bool = T::IS_IMMUTABLE;

    fn create(world: &World, id: Entity) -> Result<Self::ParamsType<'_>, GetError> {
        todo!()
    }
}

macro_rules! impl_tuple_params {
    ($($t:ident),*) => {
        impl<$($t: GetParam), *> private::Sealed for ($($t,) *) {}
        impl<$($t: GetParam), *> Params for ($($t,) *) {
            type ParamsType<'a> = ($($t::Output<'a>,) *);
            const ALL_IMMUTABLE: bool = { $($t::IS_IMMUTABLE &&)* true };

            fn create(world: &World, id: Entity) -> Result<Self::ParamsType<'_>, GetError> {
                const { assert!(Self::ALL_IMMUTABLE, "mutable access not yet supported"); }

                let id_loc = world.entity_manager.get_location(id).unwrap();
                Ok(($($t::make(world, id, id_loc)?,)*))
            }
        }
    }
}

impl_tuple_params!(P0);
impl_tuple_params!(P0, P1);
impl_tuple_params!(P0, P1, P2);
impl_tuple_params!(P0, P1, P2, P3);
impl_tuple_params!(P0, P1, P2, P3, P4);
impl_tuple_params!(P0, P1, P2, P3, P4, P5);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7, P8);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11);
impl_tuple_params!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12);
