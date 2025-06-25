use crate::type_traits::private::{SealedData, SealedTag};
use crate::{
    component::ComponentDescriptor,
    error::UnregisteredTypeErr,
    id::{Id, pair},
    registration::ComponentId,
    storage::StorageType,
    world::World,
};
use private::Sealed;
use std::marker::PhantomData;

mod private {
    pub trait Sealed {}
    pub trait SealedTag {}
    pub trait SealedData {}
}

pub trait ComponentDataType: Sealed {
    const IS_TAG: bool;
}

pub struct Tag;
pub struct Data;

impl Sealed for Tag {}
impl Sealed for Data {}

impl ComponentDataType for Data {
    const IS_TAG: bool = false;
}

impl ComponentDataType for Tag {
    const IS_TAG: bool = true;
}

pub trait PairType: Sealed {
    type Type: ComponentId;
    const IS_FIRST: bool;
}

pub struct PairTypeSelect<T: ComponentDataType, F: ComponentId, S: ComponentId> {
    marker_: PhantomData<(T, F, S)>,
}

impl<T: ComponentId, U: ComponentId> Sealed for PairTypeSelect<Data, T, U> {}

impl<T: ComponentId, U: ComponentId> PairType for PairTypeSelect<Data, T, U> {
    type Type = T;
    const IS_FIRST: bool = true;
}

impl<T: ComponentId, U: ComponentId> Sealed for PairTypeSelect<Tag, T, U> {}

impl<T: ComponentId, U: ComponentId> PairType for PairTypeSelect<Tag, T, U> {
    type Type = U;
    const IS_FIRST: bool = false;
}

pub unsafe trait Component: Sized + 'static {
    type DataType: ComponentDataType;
    type DescType: ComponentDescriptor;
    const IS_GENERIC: bool;
    const STORAGE: StorageType = StorageType::Sparse;
}

pub trait TagComponent: SealedTag {}

impl<T: Component<DataType = Tag>> SealedTag for T {}
impl<T: Component<DataType = Tag>> TagComponent for T {}
impl<T: TagComponent, U: TagComponent> SealedTag for (T, U) {}
impl<T: TagComponent, U: TagComponent> TagComponent for (T, U) {}

pub trait DataComponent: 'static + SealedData {}
impl<T: Component<DataType = Data>> SealedData for T {}
impl<T: Component<DataType = Data>> DataComponent for T {}

impl<T, U> SealedData for (T, U)
where
    T: ComponentId,
    U: ComponentId,
    (T, U): TypedId,
    <(T, U) as TypedId>::Data: DataComponent,
    PairTypeSelect<<<(T, U) as TypedId>::First as Component>::DataType, T, U>: PairType,
{
}

impl<T, U> DataComponent for (T, U)
where
    T: ComponentId,
    U: ComponentId,
    (T, U): TypedId,
    <(T, U) as TypedId>::Data: DataComponent,
    PairTypeSelect<<<(T, U) as TypedId>::First as Component>::DataType, T, U>: PairType,
{
}

pub trait TypedId: Sealed {
    type First: ComponentId;
    type Second: ComponentId;
    type Data: ComponentId;

    const IS_PAIR: bool;
    const IS_FIRST: bool;
    const IS_TAG: bool = <Self::First as Component>::DataType::IS_TAG
        && <Self::Second as Component>::DataType::IS_TAG;

    fn id(world: &World) -> Result<Id, UnregisteredTypeErr>;
}

impl<T: ComponentId + Component> Sealed for T {}
impl<T: ComponentId + Component> TypedId for T {
    type First = T;
    type Second = T;
    type Data = T;
    const IS_PAIR: bool = false;
    const IS_FIRST: bool = true;

    #[inline(always)]
    fn id(world: &World) -> Result<Id, UnregisteredTypeErr> {
        T::id(world)
    }
}

impl<T, U> Sealed for (T, U)
where
    T: ComponentId + Component,
    U: ComponentId + Component,
    PairTypeSelect<<T as Component>::DataType, T, U>: PairType,
{
}

impl<T, U> TypedId for (T, U)
where
    T: ComponentId + Component,
    U: ComponentId + Component,
    PairTypeSelect<<T as Component>::DataType, T, U>: PairType,
{
    type First = T;
    type Second = U;
    type Data = <PairTypeSelect<<T as Component>::DataType, T, U> as PairType>::Type;

    const IS_PAIR: bool = true;
    const IS_FIRST: bool = <PairTypeSelect<<T as Component>::DataType, T, U> as PairType>::IS_FIRST;

    #[inline]
    fn id(world: &World) -> Result<Id, UnregisteredTypeErr> {
        Ok(pair(T::id(world)?, U::id(world)?))
    }
}
