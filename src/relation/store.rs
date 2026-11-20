use std::ptr::NonNull;

use ahash::AHashSet;

use crate::{
    Cycles,
    data_structures::Sparse,
    id::Id,
    relation::{Cycle, Relation, Src, Targets, Tgt, registry, sources::Sources},
};

type Scratch<R> = <Cycle<R> as Cycles>::Scratch;
type TgtStore<R> = <Tgt<R> as Targets>::Store<R>;
type SrcStore<R> = Sparse<<Src<R> as Sources>::Store<R>>;

/// Resuable buffers for graph searches over one relation.
#[derive(Default)]
pub struct Walk {
    pub(crate) stack: Vec<Id>,
    pub(crate) seen: AHashSet<Id>,
}

/// The per-world data of `R` that lives outside tables.
pub(crate) struct Store<R: Relation> {
    pub(crate) targets: TgtStore<R>,
    pub(crate) sources: SrcStore<R>,
    pub(crate) scratch: Scratch<R>,
}

impl<R: Relation> Store<R> {
    fn new() -> Self {
        Self {
            targets: TgtStore::<R>::default(),
            sources: SrcStore::<R>::default(),
            scratch: Scratch::<R>::default(),
        }
    }
}

/// An owned `Store<R>` for some `R`, with its type erased.
pub(crate) struct Erased {
    ptr: NonNull<u8>,
    drop: unsafe fn(NonNull<u8>),
}

impl Erased {
    fn new<R: Relation>() -> Self {
        /// # Safety
        /// `ptr` came from `erase::<R>` and has not been dropped.
        unsafe fn drop_store<R: Relation>(ptr: NonNull<u8>) {
            drop(unsafe { Box::from_raw(ptr.cast::<Store<R>>().as_ptr()) });
        }
        Erased {
            ptr: NonNull::from(Box::leak(Box::new(Store::<R>::new()))).cast(),
            drop: drop_store::<R>,
        }
    }
}

impl Drop for Erased {
    fn drop(&mut self) {
        // SAFETY: `drop` was paired with `ptr` by `erase`.
        unsafe { (self.drop)(self.ptr) }
    }
}

/// The type-specific entry points of one relation, for paths that start
/// from an untyped `Id`.
#[derive(Debug)]
pub struct RelationVtable {
    new_store: fn() -> Erased,
}

impl RelationVtable {
    pub const fn of<R: Relation>() -> Self {
        Self { new_store: Erased::new::<R> }
    }
}

/// One store per static relation, indexed by relation id.
pub(crate) struct RelationStorage {
    items: Box<[Erased]>,
}

impl RelationStorage {
    pub(crate) fn new() -> Self {
        Self {
            items: registry().keys.iter().map(|k| (k.vtable().new_store)()).collect(),
        }
    }

    #[inline(always)]
    pub(crate) fn get<R: Relation>(&self) -> &Store<R> {
        // SAFETY: the store at `R::id()` was created by `R::KEY.fns()`, which
        // is `RelationFns::of::<R>()` per `Relation`'s contract.
        unsafe { self.items[R::id().index()].ptr.cast::<Store<R>>().as_ref() }
    }

    #[inline(always)]
    pub(crate) fn get_mut<R: Relation>(&mut self) -> &mut Store<R> {
        // SAFETY: as in `get`, and `&mut self` makes the borrow unique.
        unsafe { self.items[R::id().index()].ptr.cast::<Store<R>>().as_mut() }
    }
}
