use crate::{Id, inline_vec::InlineVec};

type Ids = InlineVec<Id, 4>;

const NONE: u32 = u32::MAX;

#[inline(always)]
fn ix(id: Id) -> usize {
    id.index() as usize
}

/// How edges relate their two ends.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Topology {
    /// Directed with a derived reverse index: both directions queryable.
    Directed {
        /// A source appears at most once in this relationship's edges.
        unique_source: bool,
        /// A target appears at most once in this relationship's edges.
        unique_target: bool,
        /// Rejects cyclical relationships.
        acyclic: bool,
        /// Creates a reverse index for (target, source) lookups.
        reverse: bool,
    },
    /// Endpoints are equivalent, which means:
    /// - One edge, stored canonically (min, max),
    /// reachable from both ends, so the secondary is the mirror half of
    /// traversal.
    /// - Acyclicity is unrepresentable: {a, b} is a 2-cycle.
    /// - Uniqueness is one property because the ends are.
    Symmetric { unique: bool },
}

impl Topology {
    #[inline]
    pub fn is_symmetric(&self) -> bool {
        matches!(self, Self::Symmetric { .. })
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
        self.sparse.get(ix(id)).and_then(|&s| (s != NONE).then_some(s as usize))
    }

    fn set_slot(&mut self, id: Id, slot: u32) {
        let idx = ix(id);
        if idx >= self.sparse.len() {
            self.sparse.resize(idx + 1, NONE);
        }

        self.sparse[idx] = slot;
    }

    #[inline(always)]
    pub(crate) fn get(&self, source: Id) -> Option<Id> {
        self.get_slot(source).map(|s| self.target[s])
    }

    #[inline(always)]
    pub fn contains(&self, source: Id, target: Id) -> bool {
        self.get(source).is_some_and(|id| id == target)
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
        self.sparse[ix(source)] = NONE;
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
        self.sparse.get(ix(id)).and_then(|&s| (s != NONE).then_some(s as usize))
    }

    fn set_slot(&mut self, id: Id, slot: u32) {
        let idx = ix(id);
        if idx >= self.sparse.len() {
            self.sparse.resize(idx + 1, NONE);
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
            self.sparse[ix(source)] = NONE;
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
        self.sparse[ix(source)] = NONE;
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

use Topology as Topo;

pub(crate) struct RelationIndex {
    topology: Topology,
    primary: Index,
    secondary: Option<Index>,
    scratch: CycleScratch,
}

impl RelationIndex {
    pub(crate) fn select(topology: Topology) -> Self {
        let (primary, secondary) = match topology {
            Topo::Directed { reverse, unique_source, unique_target, .. } => {
                (Index::new(unique_source), reverse.then(|| Index::new(unique_target)))
            }
            Topo::Symmetric { unique } => (Index::new(unique), Some(Index::new(unique))),
        };

        Self {
            topology,
            primary,
            secondary,
            scratch: CycleScratch::default(),
        }
    }

    #[inline(always)]
    pub fn topology(&self) -> Topology {
        self.topology
    }

    #[inline(always)]
    pub fn has_reverse(&self) -> bool {
        self.secondary.is_some()
    }

    /// Canonical edge orientation for symmetric relations: the stored
    /// (source, target) is (min, max) by index. Directed: identity.
    #[inline(always)]
    fn canon(&self, a: Id, b: Id) -> (Id, Id) {
        if matches!( self.topology, Topo::Symmetric { .. } if ix(b) < ix(a)) { (b, a) } else { (a, b) }
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
        match self.topology {
            Topo::Directed { .. } => !self.primary.is_empty(source),
            Topo::Symmetric { .. } => self.has_edges(source),
        }
    }

    #[inline(always)]
    pub(crate) fn has_incoming(&self, e: Id) -> bool {
        match self.topology {
            Topo::Symmetric { .. } => self.has_edges(e),
            Topo::Directed { .. } => !self
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

        match self.topology {
            Topo::Symmetric { .. } => assert!(src != tgt, "self-edge on symmetric relation"),
            Topo::Directed { acyclic, .. } => {
                if acyclic {
                    let cycle = match &self.primary {
                        Index::One(one) => Self::chain_reaches(one, tgt, src),
                        Index::Many(many) => Self::dfs_reaches(many, &mut self.scratch, tgt, src),
                    };
                    assert!(!cycle, "cycle: relating {src} -> {tgt} closes a loop");
                }
            }
        }

        // symmetric exclusive: displace old pairings
        if matches!(self.topology, Topo::Symmetric { unique: true }) {
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
            }
            // To-many: insert() dedups per-pair internally.
            Index::Many(p) => {
                if !p.insert(src, tgt) {
                    return; // duplicate pair: no-op
                }
                if let Some(sec) = &mut self.secondary {
                    sec.add(tgt, src);
                }
            }
        }

        match self.topology {
            Topo::Symmetric { unique: true } | Topo::Directed { unique_target: true, .. } => {
                self.displace_incoming(tgt, src)
            }
            _ => {}
        }
    }

    /// Chain walk for `unique_source` primaries: one successor per node,
    /// so the path is linear and cannot revisit — a repeat would already
    /// be a cycle, which the invariant forbids.
    ///
    /// O(depth). No scratch.
    fn chain_reaches(chain: &ToOne, from: Id, goal: Id) -> bool {
        let mut cur = from;
        loop {
            if cur == goal {
                return true;
            }
            match chain.get(cur) {
                Some(next) => cur = next,
                None => return false,
            }
        }
    }

    /// DFS for to-many primaries. O(V + E) over the subgraph reachable
    /// from `from` — the descendant set, not the whole relation.
    ///
    /// `seen` is required: a DAG can reach the same node by several paths,
    /// and without it a diamond re-traverses exponentially.
    fn dfs_reaches(primary: &ToMany, scratch: &mut CycleScratch, from: Id, goal: Id) -> bool {
        let CycleScratch { stack, seen } = scratch;
        stack.clear();
        seen.begin();
        stack.push(from);

        while let Some(cur) = stack.pop() {
            if cur == goal {
                return true;
            }
            if !seen.visit(cur) {
                continue;
            }
            stack.extend_from_slice(primary.get(cur));
        }
        false
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

/// Generation-stamped membership. Reset is a counter bump, not a clear,
/// so repeated DFS from a hot node costs nothing between runs.
#[derive(Default)]
struct VisitSet {
    stamp: Vec<u32>,
    version: u32,
}

impl VisitSet {
    /// Begin a traversal. Wrapping the counter would alias stale stamps
    /// with the new generation, so zero the array on the (astronomically
    /// rare) wrap and start over.
    fn begin(&mut self) {
        self.version = match self.version.checked_add(1) {
            Some(g) => g,
            None => {
                self.stamp.fill(0);
                1
            }
        };
    }

    /// Mark `id` visited. Returns true if it was not already.
    #[inline]
    fn visit(&mut self, id: Id) -> bool {
        let i = id.index() as usize;
        if i >= self.stamp.len() {
            self.stamp.resize(i + 1, 0);
        }
        let slot = &mut self.stamp[i];
        let new = *slot != self.version;
        *slot = self.version;
        new
    }
}

/// Scratch shared by cycle checks. Lives on the index so `relate` never
/// allocates.
#[derive(Default)]
struct CycleScratch {
    stack: Vec<Id>,
    seen: VisitSet,
}
