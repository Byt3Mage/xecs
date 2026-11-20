use std::{marker::PhantomData, ptr::NonNull};

use crate::{
    ComponentId, ComponentKey, HasMeta,
    bundle::{Bundle, ColumnSet},
    id::Id,
    relation::{Relation, Row, Sources, Src, Targets, Tgt},
};

/// One relation edge, containing its:
/// - opposite end
/// - position in the end's list
/// - payload (`()` means no payload).
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Edge<P: HasMeta, T: HasMeta> {
    pub(crate) id: Id,
    pub(crate) pos: P,
    pub(crate) data: T,
}

impl<P: HasMeta> Edge<P, ()> {
    #[inline(always)]
    pub(crate) fn source(id: Id, pos: P) -> Self {
        Self { id, pos, data: () }
    }
}

impl<P: HasMeta, T: HasMeta> Edge<P, T> {
    #[inline(always)]
    pub(crate) fn target(id: Id, pos: P, data: T) -> Self {
        Self { id, pos, data }
    }
}

pub(crate) type SPos<R> = <Src<R> as Sources>::Pos;
pub(crate) type TPos<R> = <Tgt<R> as Targets>::Pos;

/// One target edge in the source's targets list.
pub(crate) type Target<R> = Edge<SPos<R>, <R as Relation>::Data>;
/// One source edge in the target's sources list.
pub(crate) type Source<R> = Edge<TPos<R>, ()>;

/// The component holding `R`'s forward storage on each source.
/// The column only changes through relation operations.
pub(crate) struct ComponentOf<R>(PhantomData<fn() -> R>);

impl<R> Clone for ComponentOf<R> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<R> Copy for ComponentOf<R> {}

impl<R: Relation> ComponentOf<R> {
    #[inline(always)]
    pub(crate) fn id() -> ComponentId {
        R::KEY.data().id()
    }
}

impl<R: Relation> ColumnSet for ComponentOf<R> {
    const KEYS: &'static [&'static ComponentKey] = &[R::KEY.data()];
}

/// A value of `EdgeOf<R>`'s column, inserted as a bundle.
pub(crate) struct EdgeField<R: Relation>(pub(crate) Row<R>);

impl<R: Relation> Clone for EdgeField<R> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<R: Relation> Copy for EdgeField<R> {}

// SAFETY: one payload, of `Fwd<R>`, which `R::KEY.edge()` describes per
// `Relation`'s contract.
unsafe impl<R: Relation> Bundle for EdgeField<R> {
    type Set = ComponentOf<R>;

    #[inline(always)]
    fn write(self, mut sink: impl FnMut(NonNull<u8>)) {
        sink(NonNull::from_ref(&self.0).cast());
    }
}
