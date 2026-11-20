use std::sync::LazyLock;

use crate::{
    ComponentId, ComponentKey, HasMeta, KeySlot,
    component::{self, NULL_KEY, static_components},
    ecs::Ecs,
    id::{
        Id,
        allocator::{IdRecord, NotAlive},
    },
    macros::declare_id,
    relation::{edge::ComponentOf, store::Walk},
};

pub(crate) mod bounds;
mod directed;
mod edge;
pub(crate) mod many;
pub(crate) mod slots;
pub(crate) mod sources;
pub(crate) mod store;
pub(crate) mod targets;

pub use bounds::Unbounded;
pub use directed::Directed;
pub use slots::Slot;
pub use sources::Sources;
pub use store::RelationVtable;
pub use targets::Targets;

declare_id!(RelationId, "R");

mod sealed {
    pub trait Sealed {}
}

use sealed::Sealed;

/// Cardinality of exactly one.
pub struct One;
/// Cardinality of at most `N`.
pub struct Fixed<const N: usize>;
/// Cardinality without a bound.
pub struct Many;

/// The sources of a symmetric relation: every edge is held by both ends,
/// each with the other as its target.
pub struct Mirror;

/// Edges may form cycles, including self-edges.
pub struct Cyclic;
/// Edges may not form cycles. A self-edge is a cycle.
pub struct Acyclic;

impl Sealed for One {}
impl<const N: usize> Sealed for Fixed<N> {}
impl Sealed for Many {}
impl Sealed for Mirror {}
impl Sealed for Cyclic {}
impl Sealed for Acyclic {}

/// A relation declared with `schema!`, named by its own type.
///
/// # Safety
/// - `KEY.edge().meta()` describes `<Self::Targets as Targets>::Forward<Self::Payload>`.
/// - `KEY.fns()` is `RelationFns::of::<Self>()`.
pub unsafe trait Relation: Copy + 'static {
    /// Edges per source.
    type Targets: Targets;
    /// Edges per target.
    type Sources: Sources;
    /// Whether edges may form cycles.
    type Cycles: Cycles;
    /// Data carried by each forward edge.
    type Data: HasMeta;

    const KEY: &'static RelationKey;

    #[inline(always)]
    fn id() -> RelationId {
        Self::KEY.id()
    }
}

pub(crate) type Tgt<R> = <R as Relation>::Targets;
pub(crate) type Src<R> = <R as Relation>::Sources;
pub(crate) type Cycle<R> = <R as Relation>::Cycles;
pub(crate) type Row<R> = <Tgt<R> as Targets>::Row<R>;

/// The cycle error of `R`.
pub type CycleOf<R> = <<R as Relation>::Cycles as Cycles>::Error;

/// Error returned by `relate`. `C` and `F` are uninhabited for relations
/// that cannot fail that way.
#[derive(Debug, thiserror::Error)]
pub enum RelateError<R: Relation> {
    #[error(transparent)]
    NotAlive(#[from] NotAlive),
    #[error(transparent)]
    Cycles(<Cycle<R> as Cycles>::Error),
}

/// The edge would close a cycle.
#[derive(Debug, thiserror::Error)]
#[error("edge would close a cycle")]
pub struct CycleError;

/// Whether edges may form cycles, and the check `relate` runs.
pub trait Cycles: Sealed + 'static {
    type Scratch: Default;
    /// Error returned by `relate` when the edge would close a cycle.
    type Error: std::error::Error + 'static;

    #[doc(hidden)]
    fn check<R>(ecs: &mut Ecs, s: Id, t: Id) -> Result<(), Self::Error>
    where
        R: Relation<Cycles = Self>;
}

impl Cycles for Cyclic {
    type Scratch = ();
    type Error = !;

    #[inline(always)]
    fn check<R>(_: &mut Ecs, _: Id, _: Id) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Cycles for Acyclic {
    type Scratch = Walk;
    type Error = CycleError;

    #[inline]
    fn check<R>(ecs: &mut Ecs, src: Id, tgt: Id) -> Result<(), Self::Error>
    where
        R: Relation<Cycles = Self>,
    {
        let mut walk = std::mem::take(&mut ecs.relations.get_mut::<R>().scratch);
        walk.stack.clear();
        walk.seen.clear();
        walk.stack.push(src);

        while let Some(cur) = walk.stack.pop() {
            if cur == tgt {
                return Err(CycleError);
            }

            // With one target per source the walk is a chain, which revisits no
            // entity while the relation is acyclic.
            if !walk.seen.insert(cur) {
                continue;
            }

            let r = ecs.ids.get(cur).expect("target is alive");
            Tgt::<R>::for_each::<R>(ecs, r, |tgt| walk.stack.push(tgt.id));
        }

        ecs.relations.get_mut::<R>().scratch = walk;
        Ok(())
    }
}

/// The identity of one static relation.
#[derive(Debug)]
pub struct RelationKey {
    slot: &'static KeySlot,
    path: &'static str,
    data: &'static ComponentKey,
    vtable: &'static RelationVtable,
}

impl RelationKey {
    #[inline]
    pub const fn new(
        slot: &'static KeySlot,
        path: &'static str,
        data: &'static ComponentKey,
        vtable: &'static RelationVtable,
    ) -> Self {
        Self { slot, path, data, vtable }
    }

    /// The relation's id, identical in every world.
    ///
    /// # Panics
    /// Panics if this key is not in `STATIC_RELATIONS`.
    #[inline(always)]
    pub fn id(&self) -> RelationId {
        let mut slot = self.slot.load();
        if slot == NULL_KEY {
            slot = self.init_slot();
        }
        // SAFETY: `init_slot` only returns slots below `NULL_KEY`.
        unsafe { RelationId::new_unchecked(slot) }
    }

    #[cold]
    #[inline(never)]
    fn init_slot(&self) -> u32 {
        registry();
        let slot = self.slot.load();
        assert!(
            slot != NULL_KEY,
            "relation key `{}` is not in STATIC_RELATIONS",
            self.path
        );
        slot
    }

    #[inline]
    pub const fn path(&self) -> &'static str {
        self.path
    }

    /// The key of the column sources store this relation's edges in.
    #[inline(always)]
    pub const fn data(&self) -> &'static ComponentKey {
        self.data
    }

    #[inline(always)]
    pub(crate) const fn vtable(&self) -> &'static RelationVtable {
        self.vtable
    }
}

#[linkme::distributed_slice]
pub static STATIC_RELATIONS: [&'static RelationKey];

/// Static relations sorted by path, with each edge column mapped back to
/// its relation.
pub(crate) struct Registry {
    pub(crate) keys: Box<[&'static RelationKey]>,
    by_edge: Box<[Option<RelationId>]>,
}

pub(crate) fn registry() -> &'static Registry {
    static REGISTRY: LazyLock<Registry> = LazyLock::new(|| {
        assert!(STATIC_RELATIONS.len() < (NULL_KEY as usize), "too many relation keys");

        let mut keys: Box<_> = STATIC_RELATIONS.iter().copied().collect();
        keys.sort_unstable_by_key(|k| k.path);

        keys.windows(2)
            .for_each(|pair| assert_ne!(pair[0].path, pair[1].path, "duplicate relation path `{}`", pair[0].path));

        keys.iter().enumerate().for_each(|(i, k)| k.slot.store(i as u32));

        keys.iter().enumerate().for_each(|(i, k)| {
            assert!(
                k.slot.load() == (i as u32),
                "relation key `{}` shares its slot with another key",
                k.path
            );
        });

        let mut by_edge = vec![None; static_components().len()].into_boxed_slice();

        for (i, k) in keys.iter().enumerate() {
            // SAFETY: `i` is below `NULL_KEY`, asserted above.
            by_edge[k.data.id().index()] = Some(unsafe { RelationId::new_unchecked(i as u32) });
        }

        Registry { keys, by_edge }
    });
    &REGISTRY
}

/// The relation whose edges `component` stores, if any.
#[inline]
pub(crate) fn edge_owner(component: ComponentId) -> Option<RelationId> {
    registry().by_edge.get(component.index()).copied().flatten()
}

pub(crate) fn row<R: Relation>(ecs: &Ecs, r: IdRecord) -> Option<&Row<R>> {
    // SAFETY: `r` is live, and `ColumnOf<R>`'s column holds `Forward<R>`
    // per `Relation`'s contract.
    unsafe { component::get::<Row<R>>(&ecs.storage.tables, r, ComponentOf::<R>::id()) }
}

pub(crate) fn row_mut<R: Relation>(ecs: &mut Ecs, r: IdRecord) -> Option<&mut Row<R>> {
    // SAFETY: `r` is live, and `ColumnOf<R>`'s column holds `Forward<R>`
    // per `Relation`'s contract.
    unsafe { component::get_mut::<Row<R>>(&mut ecs.storage.tables, r, ComponentOf::<R>::id()) }
}
