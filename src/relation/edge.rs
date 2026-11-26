use std::{marker::PhantomData, ptr::NonNull};

use crate::{
    Bundle, ComponentKey, HasMeta, Id, Relation,
    bundle::ColumnSet,
    memory::{LayoutOverflow, RowLayout},
};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct Edge<T: HasMeta> {
    pub target: Id,
    pub data: T,
}

impl<T: HasMeta> Edge<T> {
    #[inline(always)]
    pub fn new(target: Id, data: T) -> Self {
        Self { target, data }
    }
}

// `target` is an `Id`: this fails to compile if the field's type changes.
#[allow(dead_code)]
fn edge_target_is_id<T: HasMeta>(edge: &Edge<T>) -> &Id {
    &edge.target
}

#[derive(Debug, Clone, Copy)]
pub struct EdgeLayout {
    pub(crate) layout: RowLayout,
    pub(crate) offset: usize,
}

impl EdgeLayout {
    #[inline(always)]
    pub const fn from_data<T: HasMeta>() -> Self {
        Self {
            layout: RowLayout::of::<Edge<T>>(),
            offset: std::mem::offset_of!(Edge<T>, data),
        }
    }

    #[inline(always)]
    pub const fn new(data: RowLayout) -> Result<Self, LayoutOverflow> {
        match RowLayout::of::<Id>().extend(data) {
            Ok((layout, offset)) => Ok(Self { layout: layout.pad_to_align(), offset }),
            Err(err) => Err(err),
        }
    }
}

pub trait EdgeFrom: HasMeta {
    const LAYOUT: &'static EdgeLayout;
}

impl<T: HasMeta> EdgeFrom for T {
    const LAYOUT: &'static EdgeLayout = &EdgeLayout::from_data::<T>();
}

/// The component holding `R`'s forward storage on each source.
/// The column only changes through relation operations.
pub struct ComponentOf<R: Relation>(PhantomData<fn() -> R>);

impl<R: Relation> Copy for ComponentOf<R> {}
impl<R: Relation> Clone for ComponentOf<R> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<R: Relation> ColumnSet for ComponentOf<R> {
    const KEYS: &'static [&'static ComponentKey] = &[R::KEY.component()];
}

/// A value of `Edge<R::Data>`'s column, inserted as a bundle.
pub(crate) struct EdgeField<R: Relation>(pub(crate) Edge<R::Data>);

impl<R: Relation> Copy for EdgeField<R> {}
impl<R: Relation> Clone for EdgeField<R> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

// SAFETY: one payload, of `Edge<R>`, which `R::KEY.component()` describes per
// `Relation`'s contract.
unsafe impl<R: Relation> Bundle for EdgeField<R> {
    type Set = ComponentOf<R>;

    #[inline(always)]
    fn write(self, mut sink: impl FnMut(NonNull<u8>)) {
        sink(NonNull::from_ref(&self.0).cast());
    }
}
