use crate::{Id, inline_vec::InlineVec};

type Ids = InlineVec<Id, 4>;

pub(crate) const SPARSE_NONE: u32 = u32::MAX;

#[inline(always)]
fn ix(id: Id) -> usize {
    id.index() as usize
}

/// How edges relate their two ends.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Topology {
    /// Directed, forward-only. Reversed queries are plan-time errors;
    /// purge scans the forward shape.
    Directed,
    /// Directed with a derived reverse index: both directions queryable.
    DirectedIndexed,
    /// Undirected: one edge, endpoints equivalent, stored canonically
    /// (min, max). The secondary index is the mirror half of traversal.
    Symmetric,
}

/// Declaration-time validation. Invalid combinations are rejected here;
/// storage selection below is total over what passes.
#[derive(thiserror::Error, Debug)]
pub enum SymmetryError {
    #[error("symmetric relation cannot be acyclic: any edge {{a, b}} is a 2-cycle")]
    IsAcyclic,
    #[error("symmetric relation requires unique_source == unique_target")]
    UniquenessMismatch,
}

#[derive(Clone, Copy, Debug)]
pub struct RelationProperties {
    /// A source appears at most once in this relationship's edges.
    pub unique_source: bool,
    /// A target appears at most once in this relationship's edges.
    pub unique_target: bool,
    pub topology: Topology,
    pub acyclic: bool,
}

impl RelationProperties {
    pub fn validate(&self) -> Result<(), SymmetryError> {
        if self.topology == Topology::Symmetric {
            if self.unique_source != self.unique_target {
                return Err(SymmetryError::UniquenessMismatch);
            }
            if self.acyclic {
                return Err(SymmetryError::IsAcyclic);
            }
        }
        Ok(())
    }
}

/// At most one edge per key. Dense arrays are position-parallel;
/// removal swap-pops and fixes the moved entry's sparse slot.
pub struct ToOne {
    sparse: Vec<u32>,
    pub(crate) source: Vec<Id>,
    pub(crate) target: Vec<Id>,
}

impl ToOne {
    pub fn new() -> Self {
        Self { sparse: vec![], source: vec![], target: vec![] }
    }

    fn get_slot(&self, id: Id) -> Option<usize> {
        self.sparse
            .get(ix(id))
            .and_then(|&s| (s != SPARSE_NONE).then_some(s as usize))
    }

    fn set_slot(&mut self, id: Id, slot: u32) {
        let idx = ix(id);
        if idx >= self.sparse.len() {
            self.sparse.resize(idx + 1, SPARSE_NONE);
        }

        self.sparse[idx] = slot;
    }

    #[inline(always)]
    pub(crate) fn get(&self, source: Id) -> Option<Id> {
        self.get_slot(source).map(|s| self.target[s])
    }

    #[inline(always)]
    pub fn contains(&self, source: Id, target: Id) -> bool {
        self.get(source) == Some(target)
    }

    /// Insert or retarget. Returns the previous target on retarget —
    /// the caller (RelationStorage) uses it to maintain the reverse.
    pub(crate) fn set(&mut self, source: Id, target: Id) -> Option<Id> {
        match self.get_slot(source) {
            Some(s) => Some(std::mem::replace(&mut self.target[s], target)),
            None => {
                let slot = self.source.len() as u32;
                self.source.push(source);
                self.target.push(target);
                self.set_slot(source, slot);
                None
            }
        }
    }

    /// Remove source's edge. Returns its target if edge existed.
    pub(crate) fn remove(&mut self, source: Id) -> Option<Id> {
        let slot = self.get_slot(source)? as usize;
        self.sparse[ix(source)] = SPARSE_NONE;
        let target = self.target.swap_remove(slot);
        self.source.swap_remove(slot);
        if slot < self.source.len() {
            self.sparse[ix(self.source[slot])] = slot as u32;
        }
        Some(target)
    }
}

/// Multiple edges per key: per-key value lists behind the sparse set.
/// List positions are private to the entry; removal never propagates.
pub struct ToMany {
    sparse: Vec<u32>,
    pub(crate) source: Vec<Id>,
    pub(crate) targets: Vec<Ids>,
}

impl ToMany {
    pub(crate) fn new() -> Self {
        Self { sparse: vec![], source: vec![], targets: vec![] }
    }

    fn get_slot(&self, id: Id) -> Option<usize> {
        self.sparse
            .get(ix(id))
            .and_then(|&s| (s != SPARSE_NONE).then_some(s as usize))
    }

    fn set_slot(&mut self, id: Id, slot: u32) {
        let idx = ix(id);
        if idx >= self.sparse.len() {
            self.sparse.resize(idx + 1, SPARSE_NONE);
        }

        self.sparse[idx] = slot;
    }

    #[inline(always)]
    pub(crate) fn get(&self, source: Id) -> &[Id] {
        self.get_slot(source).map_or(&[], |s| &self.targets[s])
    }

    #[inline(always)]
    pub(crate) fn contains(&self, source: Id, target: Id) -> bool {
        self.get(source).contains(&target)
    }

    /// Insert (source -> target). Per-pair unique: re-inserting an
    /// existing pair is a no-op. Returns true if the edge is new.
    pub(crate) fn insert(&mut self, source: Id, target: Id) -> bool {
        match self.get_slot(source) {
            Some(slot) => {
                let list = &mut self.targets[slot];
                if list.contains(&target) {
                    return false;
                }
                list.push(target);
                true
            }
            None => {
                let slot = self.source.len() as u32;
                self.source.push(source);
                self.targets.push(Ids::from([target]));
                self.set_slot(source, slot);
                true
            }
        }
    }

    /// Remove one edge. Empty entries are swap-popped so the dense
    /// arrays stay dense. Returns true if the edge existed.
    pub(crate) fn remove(&mut self, source: Id, target: Id) -> bool {
        let Some(slot) = self.get_slot(source) else { return false };
        let slot = slot as usize;
        let list = &mut self.targets[slot];
        let Some(at) = list.iter().position(|&t| t == target) else { return false };
        list.swap_remove(at);

        if list.is_empty() {
            self.sparse[ix(source)] = SPARSE_NONE;
            self.source.swap_remove(slot);
            self.targets.swap_remove(slot);
            if slot < self.source.len() {
                self.sparse[ix(self.source[slot])] = slot as u32;
            }
        }
        true
    }

    /// Remove an entire entry (purge helper). Returns its target list.
    pub(crate) fn remove_all(&mut self, source: Id) -> Ids {
        let Some(slot) = self.get_slot(source) else { return Ids::new() };
        let slot = slot as usize;
        self.sparse[ix(source)] = SPARSE_NONE;
        self.source.swap_remove(slot);
        let list = self.targets.swap_remove(slot);
        if slot < self.source.len() {
            self.sparse[ix(self.source[slot])] = slot as u32;
        }
        list
    }
}

pub(crate) enum Index {
    One(ToOne),
    Many(ToMany),
}

impl Index {
    fn new(unique: bool) -> Self {
        if unique { Index::One(ToOne::new()) } else { Index::Many(ToMany::new()) }
    }

    /// One key's edges in this direction.
    #[inline(always)]
    pub(crate) fn get(&self, key: Id) -> Edges<'_> {
        match self {
            Index::One(i) => Edges::One(i.get(key)),
            Index::Many(i) => Edges::Many(i.get(key)),
        }
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self, key: Id) -> bool {
        match self {
            Index::One(i) => i.get_slot(key).is_none(),
            Index::Many(i) => i.get_slot(key).is_none(),
        }
    }

    #[inline(always)]
    fn contains(&self, key: Id, value: Id) -> bool {
        match self {
            Index::One(i) => i.contains(key, value),
            Index::Many(i) => i.contains(key, value),
        }
    }

    fn add(&mut self, key: Id, value: Id) -> bool {
        match self {
            Index::One(i) => {
                debug_assert!(i.get(key).is_none_or(|v| v == value), "uniqueness violated");
                i.set(key, value) != Some(value)
            }
            Index::Many(i) => i.insert(key, value),
        }
    }

    fn remove(&mut self, key: Id, value: Id) -> bool {
        match self {
            Index::One(i) => match i.get(key) {
                Some(v) if v == value => {
                    i.remove(key);
                    true
                }
                _ => false,
            },
            Index::Many(i) => i.remove(key, value),
        }
    }

    /// Remove all edges of `key`, returning the far ends.
    fn remove_all(&mut self, key: Id) -> Ids {
        match self {
            Index::One(i) => i.remove(key).into_iter().collect(),
            Index::Many(i) => i.remove_all(key),
        }
    }
}

#[derive(Default)]
pub(crate) struct TreeExtras {
    depth: Vec<u16>, // per dense slot
    topo: Vec<Id>,   // parents-before-children, lazy
    topo_dirty: bool,
}

pub(crate) struct RelationIndex {
    props: RelationProperties,
    primary: Index,
    secondary: Option<Index>,
}

impl RelationIndex {
    pub(crate) fn select(props: RelationProperties) -> Result<Self, SymmetryError> {
        props.validate()?;

        let primary = Index::new(props.unique_source);
        let secondary = match props.topology {
            Topology::Directed => None,
            Topology::DirectedIndexed | Topology::Symmetric => Some(Index::new(props.unique_target)),
        };

        Ok(Self { props, primary, secondary })
    }

    #[inline(always)]
    pub fn props(&self) -> RelationProperties {
        self.props
    }

    #[inline(always)]
    pub fn has_reverse(&self) -> bool {
        self.secondary.is_some()
    }

    /// Canonical edge orientation for symmetric relations: the stored
    /// (source, target) is (min, max) by index. Directed: identity.
    #[inline(always)]
    fn canon(&self, a: Id, b: Id) -> (Id, Id) {
        if self.props.topology == Topology::Symmetric && ix(b) < ix(a) { (b, a) } else { (a, b) }
    }

    /// Outgoing targets of `id`. Symmetric: canonical half of id's
    /// neighbors (chain with incoming() for the full set).
    #[inline]
    pub(crate) fn outgoing(&self, id: Id) -> Edges<'_> {
        self.primary.get(id)
    }

    /// Incoming sources of `id` via the reverse (symmetric: the mirror
    /// half). Lowering guarantees this is only reached when it exists.
    #[inline]
    pub(crate) fn incoming(&self, id: Id) -> Edges<'_> {
        self.secondary
            .as_ref()
            .expect("INTERNAL ERROR: incoming without secondary index")
            .get(id)
    }

    #[inline]
    pub(crate) fn has_outgoing(&self, source: Id) -> bool {
        match self.props.topology {
            Topology::Symmetric => self.has_edges(source),
            _ => !self.primary.is_empty(source),
        }
    }

    #[inline(always)]
    pub(crate) fn has_incoming(&self, e: Id) -> bool {
        match self.props.topology {
            Topology::Symmetric => self.has_edges(e),
            _ => !self
                .secondary
                .as_ref()
                .expect("incoming probe without secondary index: lowering bug")
                .is_empty(e),
        }
    }

    /// Symmetric existence: any edge, either half.
    #[inline(always)]
    fn has_edges(&self, e: Id) -> bool {
        !self.primary.is_empty(e) || self.secondary.as_ref().is_some_and(|s| !s.is_empty(e))
    }

    /// Does the edge exist? Symmetric: orientation-free.
    #[inline]
    pub(crate) fn contains(&self, source: Id, target: Id) -> bool {
        let (s, t) = self.canon(source, target);
        self.primary.contains(s, t)
    }

    /// Create the edge source -> target (symmetric: {a, b}).
    /// Uniqueness replaces: displaced edges are removed in the same
    /// motion, in-place where the shape allows.
    pub(crate) fn relate(&mut self, source: Id, target: Id) {
        let (src, tgt) = self.canon(source, target);

        if self.props.topology == Topology::Symmetric {
            assert!(src != tgt, "self-edge on symmetric relation");
        }

        if self.props.acyclic {
            if let Index::One(chain) = &self.primary {
                let mut cur = tgt;
                loop {
                    assert!(cur != src, "cycle: relating {src} -> {tgt} closes a loop");
                    match chain.get(cur) {
                        Some(next) => cur = next,
                        None => break,
                    }
                }
            }
        }

        // symmetric exclusive: displace old pairings
        if self.props.topology == Topology::Symmetric && self.props.unique_source {
            if self.contains(src, tgt) {
                return;
            }
            self.displace_exclusive_pairing(src);
            self.displace_exclusive_pairing(tgt);
        }

        match &mut self.primary {
            // To-one: set() IS insert-or-retarget in place.
            // Secondary follows the returned old value.
            Index::One(p) => {
                if let Some(old) = p.set(src, tgt) {
                    if old == tgt {
                        return; // same edge re-related: no-op
                    }

                    if let Some(sec) = &mut self.secondary {
                        sec.remove(old, src);
                        sec.add(tgt, src);
                    }
                }

                // unique_target on a to-one primary: displace tgt's old
                // source (if different) via the secondary.
                if self.props.unique_target {
                    self.displace_incoming(tgt, src);
                }
            }
            // To-many: insert() dedups per-pair internally.
            Index::Many(p) => {
                if !p.insert(src, tgt) {
                    return; // duplicate pair: no-op
                }
                if let Some(sec) = &mut self.secondary {
                    sec.add(tgt, src);
                }
                if self.props.unique_target {
                    self.displace_incoming(tgt, src);
                }
            }
        }
    }

    /// Remove tgt's incoming edge from any source other than `keep`.
    /// unique_target displacement: the new source evicts the old.
    fn displace_incoming(&mut self, tgt: Id, keep: Id) {
        let old = match &self.secondary {
            Some(Index::One(s)) => s.get(tgt).filter(|&o| o != keep),
            Some(Index::Many(_)) => unreachable!("unique_target with Many secondary: select bug"),
            // Plain Directed + unique_target: scan (declared trade).
            None => match &self.primary {
                Index::One(p) => p
                    .source
                    .iter()
                    .zip(&p.target)
                    .find(|&(&k, &v)| v == tgt && k != keep)
                    .map(|(&k, _)| k),
                Index::Many(p) => p
                    .source
                    .iter()
                    .zip(&p.targets)
                    .find(|&(&k, l)| l.contains(&tgt) && k != keep)
                    .map(|(&k, _)| k),
            },
        };

        if let Some(old_src) = old {
            self.unrelate(old_src, tgt);
        }
    }

    fn displace_exclusive_pairing(&mut self, id: Id) {
        let partner = match self.primary.get(id) {
            Edges::One(p) => p,
            Edges::Many(_) => unreachable!("exclusive symmetric selects One shapes: select bug"),
        }
        .or_else(|| match self.secondary.as_ref().map(|s| s.get(id)) {
            Some(Edges::One(p)) => p,
            Some(Edges::Many(_)) => unreachable!("exclusive symmetric selects One shapes: select bug"),
            None => unreachable!("symmetric always has a secondary: select bug"),
        });

        if let Some(p) = partner {
            self.unrelate(id, p);
        }
    }

    /// Remove the edge, if present.
    pub(crate) fn unrelate(&mut self, source: Id, target: Id) {
        let (src, tgt) = self.canon(source, target);
        if self.primary.remove(src, tgt) {
            if let Some(sec) = &mut self.secondary {
                sec.remove(tgt, src);
            }
        }
    }

    /// Despawn cascade: every edge touching `id`, both roles.
    pub(crate) fn purge(&mut self, id: Id) {
        // id as stored source.
        for t in self.primary.remove_all(id) {
            if let Some(sec) = &mut self.secondary {
                sec.remove(t, id);
            }
        }

        // id as stored target.
        let sources: Ids = match &mut self.secondary {
            Some(sec) => sec.remove_all(id),
            // Directed without index: scan.
            // The documented cost of Topology::Directed.
            None => match &self.primary {
                Index::One(p) => p
                    .source
                    .iter()
                    .zip(&p.target)
                    .filter_map(|(&s, &t)| (t == id).then_some(s))
                    .collect(),
                Index::Many(p) => p
                    .source
                    .iter()
                    .zip(&p.targets)
                    .filter_map(|(&s, t)| t.contains(&id).then_some(s))
                    .collect(),
            },
        };

        for s in sources {
            self.primary.remove(s, id);
        }
    }
}

/// A read of one entity's edges in one direction. One shape yields at
/// most one id; Many yields a slice. Callers (fans) flatten this.
pub(crate) enum Edges<'a> {
    One(Option<Id>),
    Many(&'a [Id]),
}
