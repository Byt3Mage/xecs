use crate::{
    Id, TypeMeta,
    data_structures::{Sparse, VecIdxU32},
    inline_vec::InlineVec,
    invec,
    memory::RowMeta,
    relation::{
        directed::Directed,
        edges::{ManyEdges, OneEdges},
        hierarchy::{ChildIter, Hierarchy},
        symmetry::Symmetry,
    },
};

type Ids = InlineVec<Id, 4>;

/// How edges relate their two ends.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Directed with a derived reverse index: both directions queryable.
    Directed {
        /// An [Id] appears as a source at most once in the relationship's edges.
        unique_source: bool,
        /// An [Id] appears as a target at most once in the relationship's edges.
        unique_target: bool,
        /// Rejects cyclical relationships.
        acyclic: bool,
        /// Creates a reverse index for (target, source) lookups.
        reverse: bool,
    },

    /// Endpoints are equivalent, which means acyclicity
    /// is unrepresentable ({a, b} is a 2-cycle).
    Symmetric { unique: bool },

    /// Directed with a tree index.
    Hierarchical,
}

impl Default for Shape {
    /// Default topology is a many to many non-acyclic relationship
    /// with no reverse index.
    fn default() -> Self {
        Shape::Directed {
            unique_source: false,
            unique_target: false,
            acyclic: false,
            reverse: false,
        }
    }
}

impl Shape {
    #[inline]
    pub fn is_symmetric(&self) -> bool {
        matches!(self, Self::Symmetric { .. })
    }
}

pub(super) enum Target {
    One(OneEdges),
    Many(VecIdxU32<ManyEdges>),
}

/// A read of one entity's edges in one direction.
/// `One` yields at most one id. `Many` yields a slice.
/// 'Children` yields the subtree of the hierarchy.
pub(crate) enum Edges<'a> {
    One(Id),
    Many(&'a [Id]),
    Children(ChildIter<'a>),
}

impl<'a> Edges<'a> {
    pub fn into_iter(self) -> EdgeIter<'a> {
        match self {
            Edges::One(id) => EdgeIter::One(Some(id)),
            Edges::Many(ids) => EdgeIter::Many(ids.iter()),
            Edges::Children(c) => EdgeIter::Children(c),
        }
    }
}

pub(crate) enum EdgeIter<'a> {
    One(Option<Id>),
    Many(std::slice::Iter<'a, Id>),
    Children(ChildIter<'a>),
}
impl<'a> EdgeIter<'a> {
    #[inline]
    pub const fn empty() -> Self {
        Self::One(None)
    }
}

impl Iterator for EdgeIter<'_> {
    type Item = Id;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            EdgeIter::One(v) => v.take(),
            EdgeIter::Many(it) => it.next().copied(),
            EdgeIter::Children(c) => c.next(),
        }
    }
}

pub(super) struct Index {
    pub(super) sparse: Sparse<u32>,
    pub(super) source: VecIdxU32<Id>,
    pub(super) target: Target,
    meta: RowMeta,
}

impl Drop for Index {
    fn drop(&mut self) {
        match &mut self.target {
            Target::One(e) => e.dispose(self.meta),
            Target::Many(many) => many.iter_mut().for_each(|l| {
                l.dispose(self.meta);
            }),
        }
    }
}

impl Index {
    pub(crate) fn new(unique: bool, meta: &TypeMeta) -> Self {
        let meta = RowMeta::new(meta.layout, meta.drop);
        Self {
            sparse: Sparse::new(),
            source: VecIdxU32::new(),
            target: match unique {
                true => Target::One(OneEdges::new(meta)),
                false => Target::Many(VecIdxU32::new()),
            },
            meta,
        }
    }
}

impl Index {
    #[inline(always)]
    pub(crate) fn is_empty(&self, id: Id) -> bool {
        !self.sparse.contains(id)
    }

    /// One id's edges in this direction.
    #[inline(always)]
    pub(crate) fn get(&self, id: Id) -> Option<Edges<'_>> {
        self.sparse.get(id).map(|&s| match &self.target {
            Target::One(e) => Edges::One(e.target(s)),
            Target::Many(e) => Edges::Many(e[s].targets()),
        })
    }

    /// One id's edges in this direction.
    #[inline(always)]
    pub(crate) fn get_unique(&self, id: Id) -> Option<Id> {
        self.sparse.get(id).and_then(|&s| match &self.target {
            Target::One(e) => Some(e.target(s)),
            Target::Many { .. } => None,
        })
    }

    #[inline(always)]
    pub(crate) fn contains(&self, source: Id, target: Id) -> bool {
        self.sparse.get(source).is_some_and(|&s| match &self.target {
            Target::One(e) => e.target(s) == target,
            Target::Many(e) => e[s].contains(target),
        })
    }

    /// Insert `(source, target)`.
    ///
    /// Returns false and drops `payload` with the existing value
    /// if the pair already exists.
    ///
    /// # Safety
    /// `T` is the relation's declared payload type.
    pub(super) unsafe fn add<T>(&mut self, source: Id, target: Id, payload: T) -> bool {
        let meta = self.meta;

        if let Some(&slot) = self.sparse.get(source) {
            return match &mut self.target {
                Target::One(e) => {
                    if e.target(slot) == target {
                        return false;
                    }
                    // SAFETY: caller guarantees `T`; the slot is live.
                    unsafe { e.replace(meta, slot, target, payload) };
                    true
                }
                Target::Many(e) => {
                    let list = &mut e[slot];
                    if list.contains(target) {
                        return false;
                    }
                    // SAFETY: caller guarantees `T`.
                    unsafe { list.push(meta, target, payload) };
                    true
                }
            };
        }

        let slot = self.source.len();
        self.source.push(source);
        // SAFETY: caller guarantees `T`.
        match &mut self.target {
            Target::One(e) => unsafe { e.push(meta, target, payload) },
            Target::Many(e) => {
                let mut many = ManyEdges::new(meta);
                unsafe { many.push(meta, target, payload) };
                e.push(many);
            }
        }
        self.sparse.set(source, slot);
        true
    }

    pub(crate) fn remove(&mut self, source: Id, target: Id) -> bool {
        let Some(&slot) = self.sparse.get(source) else { return false };
        let meta = self.meta;

        match &mut self.target {
            Target::One(e) => {
                if e.target(slot) != target {
                    return false;
                }

                self.sparse.remove(source);
                self.source.swap_remove(slot);
                e.swap_remove(meta, slot);
            }
            Target::Many(e) => {
                let list = &mut e[slot];
                let Some(at) = list.position(target) else { return false };
                list.swap_remove(meta, at);

                if list.len() == 0 {
                    self.sparse.remove(source);
                    self.source.swap_remove(slot);
                    e.swap_remove(slot).dispose(meta);
                }
            }
        }

        if slot < self.source.len() {
            self.sparse.set(self.source[slot], slot);
        }
        true
    }

    /// Remove all edges of `source`, returning all its targets.
    pub(crate) fn remove_all(&mut self, source: Id) -> Ids {
        let Some(&slot) = self.sparse.get(source) else { return invec![] };
        let meta = self.meta;

        self.sparse.remove(source);
        self.source.swap_remove(slot);

        let ids = match &mut self.target {
            Target::One(e) => std::iter::once(e.swap_remove(meta, slot)).collect(),
            Target::Many(e) => e.swap_remove(slot).dispose(meta),
        };

        if slot < self.source.len() {
            self.sparse.set(self.source[slot], slot);
        }

        ids
    }

    /// The value on `(source, target)`.
    ///
    /// # Safety
    /// `T` is the relation's declared payload type.
    pub(crate) unsafe fn payload<T>(&self, source: Id, target: Id) -> Option<&T> {
        let &slot = self.sparse.get(source)?;
        let meta = self.meta;
        match &self.target {
            Target::One(e) => (e.target(slot) == target).then(|| unsafe { e.value(meta, slot) }),
            Target::Many(e) => e[slot].position(target).map(|i| unsafe { e[slot].value(meta, i) }),
        }
    }

    /// The value on `(source, target)`.
    ///
    /// # Safety
    /// `T` is the relation's declared payload type.
    pub(crate) unsafe fn payload_mut<T>(&mut self, source: Id, target: Id) -> Option<&mut T> {
        let &slot = self.sparse.get(source)?;
        let meta = self.meta;
        match &mut self.target {
            Target::One(e) => (e.target(slot) == target).then(|| unsafe { e.value_mut(meta, slot) }),
            Target::Many(e) => e[slot].position(target).map(|i| unsafe { e[slot].value_mut(meta, i) }),
        }
    }

    /// Collect every key whose values include `value`.
    ///
    /// A linear scan of the dense arrays. This is the cost of a
    /// directed relation declared without a reverse index, where the
    /// answer would otherwise be one lookup. Only `purge` reaches this;
    /// unpinned reversed queries are rejected at plan-build time rather
    /// than falling back to a scan.
    ///
    /// Appends to `out` rather than returning, so callers can hold the
    /// buffer as scratch and never allocate.
    pub(crate) fn find_sources(&self, target: Id, out: &mut Vec<Id>) {
        match &self.target {
            Target::One(e) => out.extend(
                self.source
                    .iter()
                    .zip(e.targets())
                    .filter_map(|(&k, &v)| (v == target).then_some(k)),
            ),
            Target::Many(e) => out.extend(
                self.source
                    .iter()
                    .zip(e.iter())
                    .filter_map(|(&k, vs)| vs.contains(target).then_some(k)),
            ),
        }
    }
}

pub(crate) enum Storage {
    Directed(Directed),
    Symmetry(Symmetry),
    Hierarchy(Hierarchy),
}

impl Storage {
    pub fn select(shape: Shape, meta: &TypeMeta) -> Self {
        match shape {
            Shape::Hierarchical => Storage::Hierarchy(Hierarchy::new()),
            Shape::Symmetric { unique } => Storage::Symmetry(Symmetry::new(unique, meta)),
            Shape::Directed { unique_source, unique_target, acyclic, reverse } => {
                Storage::Directed(Directed::new(unique_source, unique_target, acyclic, reverse, meta))
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RelateError {
    #[error("{0} forms self-edge on a symmetric relation")]
    SelfEdgeOnSymmetric(Id),

    #[error("{0} forms self-edge on a hierarchical relation")]
    SelfEdgeOnHierarchy(Id),

    #[error("{0} -> {1} closes a loop on an acyclic relation")]
    CycleOnAcyclic(Id, Id),
}
