use std::sync::LazyLock;

use crate::{
    ComponentId, ComponentKey, HasMeta, NotAlive,
    component::{self, static_components},
    data_structures::Sparse,
    id::{Id, allocator::IdRecord},
    macros::declare_id,
    memory::LayoutOverflow,
    relation::{
        edge::{Edge, EdgeLayout},
        many::Many,
    },
    table_storage::Tables,
    utils::KeySlot,
};

mod directed;
pub(crate) mod edge;
pub(crate) mod many;
pub(crate) mod registry;

pub use edge::EdgeFrom;

pub(crate) use directed::relate;

declare_id!(RelationId, "R");

/// A relation declared with `schema!`, named by its own type.
///
/// # Safety
/// - `KEY.edge().meta()` describes `<Self::Targets as Targets>::Forward<Self::Payload>`.
/// - `KEY.fns()` is `RelationFns::of::<Self>()`.
pub unsafe trait Relation: Copy + 'static {
    /// Data carried by each forward edge.
    type Data: HasMeta;
    const UNIQUE_TARGET: bool;
    const UNIQUE_SOURCE: bool;
    const DIRECTION: Direction;
    const KEY: &'static RelationKey;
}

#[inline(always)]
pub fn id<R: Relation>() -> RelationId {
    R::KEY.id()
}

/// The identity of one static relation.
#[derive(Debug)]
pub struct RelationKey {
    props: Props,
    slot: &'static KeySlot,
    path: &'static str,
    edge: &'static EdgeLayout,
    component: &'static ComponentKey,
}

impl RelationKey {
    #[inline]
    pub const fn new(
        props: Props,
        slot: &'static KeySlot,
        path: &'static str,
        edge: &'static EdgeLayout,
        component: &'static ComponentKey,
    ) -> Self {
        Self { props, slot, path, edge, component }
    }

    /// The relation's id, identical in every world.
    ///
    /// # Panics
    /// Panics if this key is not in `STATIC_RELATIONS`.
    #[inline(always)]
    pub fn id(&self) -> RelationId {
        match RelationId::from_raw(self.slot()) {
            Some(r) => r,
            None => self.init_id(),
        }
    }

    #[cold]
    #[inline(never)]
    fn init_id(&self) -> RelationId {
        static_relations();
        match RelationId::from_raw(self.slot()) {
            Some(r) => r,
            None => panic!("key `{}` is not in STATIC_RELATIONS", self.path),
        }
    }

    #[inline(always)]
    pub fn slot(&self) -> u32 {
        self.slot.load()
    }

    #[inline]
    pub const fn props(&self) -> Props {
        self.props
    }

    #[inline]
    pub const fn path(&self) -> &'static str {
        self.path
    }

    #[inline(always)]
    pub const fn data(&self) -> &'static EdgeLayout {
        self.edge
    }

    /// The key of the column sources store this relation's edges in.
    #[inline(always)]
    pub const fn component(&self) -> &'static ComponentKey {
        self.component
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

pub(crate) fn static_relations() -> &'static Registry {
    static REGISTRY: LazyLock<Registry> = LazyLock::new(|| {
        assert!(STATIC_RELATIONS.len() < (u32::MAX as usize), "too many relation keys");

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
            by_edge[k.component.id().index()] = Some(unsafe { RelationId::new_unchecked(i as u32) });
        }

        Registry { keys, by_edge }
    });
    &REGISTRY
}

/// The relation whose edges `component` stores, if any.
#[inline]
pub(crate) fn edge_owner(component: ComponentId) -> Option<RelationId> {
    static_relations().by_edge.get(component.index()).copied().flatten()
}

#[derive(Debug, thiserror::Error)]
pub enum RelateError {
    #[error(transparent)]
    NotAlive(#[from] NotAlive),
    #[error("edge formed by {src} -> {tgt} would close a cycle")]
    Cycles { src: Id, tgt: Id },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Direction {
    Cyclic,
    Acyclic,
    Symmetric,
}

#[derive(Debug, Clone, Copy)]
pub struct Props {
    one_target: bool,
    one_source: bool,
    direction: Direction,
}

pub enum Targets {
    One,
    Many(Sparse<Many>),
}

pub(crate) enum Sources {
    One(Sparse<Id>),
    Many(Sparse<Vec<Id>>),
}

pub struct RelationInfo {
    pub(crate) targets: Targets,
    pub(crate) sources: Sources,
    pub(crate) direction: Direction,
    pub(crate) edge: EdgeLayout,
    pub(crate) component: ComponentId,
}

impl RelationInfo {
    fn from_key(key: &RelationKey) -> Result<Self, LayoutOverflow> {
        let targets = if key.props.one_target { Targets::One } else { Targets::Many(Sparse::new()) };
        let sources = if key.props.one_source { Sources::One(Sparse::new()) } else { Sources::Many(Sparse::new()) };
        Ok(Self {
            targets,
            sources,
            direction: key.props.direction,
            edge: *key.edge,
            component: key.component().id(),
        })
    }
}

#[inline(always)]
fn target<T: HasMeta>(tables: &Tables, r: IdRecord, comp: ComponentId) -> Option<&Edge<T>> {
    unsafe { component::get::<Edge<T>>(tables, r, comp) }
}

#[inline(always)]
fn target_mut<T: HasMeta>(tables: &mut Tables, r: IdRecord, comp: ComponentId) -> Option<&mut Edge<T>> {
    unsafe { component::get_mut::<Edge<T>>(tables, r, comp) }
}
