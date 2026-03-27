use private::Sealed;
use std::marker::PhantomData;

use crate::{
    component::Component,
    error::MissingType,
    id::{Entity, Id, pair::Pair},
    world::World,
};

mod private {
    pub trait Sealed {}
}

pub trait ComponentType: Sealed {
    const IS_TAG: bool;
}

pub struct Tag;
pub struct Data;

impl Sealed for Tag {}
impl Sealed for Data {}

impl ComponentType for Tag {
    const IS_TAG: bool = true;
}

impl ComponentType for Data {
    const IS_TAG: bool = false;
}

pub trait PairType: Sealed {
    type Type: Component;
    const IS_FIRST: bool;
}

pub struct PairTypeSelect<T: ComponentType, F: Component, S: Component> {
    marker_: PhantomData<(T, F, S)>,
}

impl<T: Component, U: Component> Sealed for PairTypeSelect<Data, T, U> {}
impl<T: Component, U: Component> PairType for PairTypeSelect<Data, T, U> {
    type Type = T;
    const IS_FIRST: bool = true;
}

impl<T: Component, U: Component> Sealed for PairTypeSelect<Tag, T, U> {}
impl<T: Component, U: Component> PairType for PairTypeSelect<Tag, T, U> {
    type Type = U;
    const IS_FIRST: bool = false;
}

pub trait TypedId: Sealed {
    type First: Component;
    type Second: Component;
    type Data: Component;
    type Id: Id;

    const IS_PAIR: bool;
    const IS_FIRST: bool;
    const IS_TAG: bool = <Self::First as Component>::DataType::IS_TAG
        && <Self::Second as Component>::DataType::IS_TAG;

    fn id(world: &World) -> Result<Self::Id, MissingType>;
}

impl<T: Component> Sealed for T {}
impl<T: Component> TypedId for T {
    type First = T;
    type Second = T;
    type Data = T;
    type Id = Entity;

    const IS_PAIR: bool = false;
    const IS_FIRST: bool = true;

    fn id(world: &World) -> Result<Self::Id, MissingType> {
        T::id(world).ok_or_else(MissingType::new::<T>)
    }
}

impl<T, U> Sealed for (T, U)
where
    T: Component,
    U: Component,
    PairTypeSelect<<T as Component>::DataType, T, U>: PairType,
{
}

impl<T, U> TypedId for (T, U)
where
    T: Component,
    U: Component,
    PairTypeSelect<<T as Component>::DataType, T, U>: PairType,
{
    type First = T;
    type Second = U;
    type Data = <PairTypeSelect<<T as Component>::DataType, T, U> as PairType>::Type;
    type Id = Pair;

    const IS_PAIR: bool = true;
    const IS_FIRST: bool = <PairTypeSelect<<T as Component>::DataType, T, U> as PairType>::IS_FIRST;

    fn id(world: &World) -> Result<Self::Id, MissingType> {
        Ok(Pair {
            rel: T::id(world).ok_or_else(MissingType::new::<T>)?,
            tgt: U::id(world).ok_or_else(MissingType::new::<U>)?,
        })
    }
}
