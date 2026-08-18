use crate::{
    Id, TypeMeta,
    relation::storage::{Edges, Index, RelateError, Target},
    type_meta::HasMeta,
};

/// Generation-stamped membership. Reset is a counter bump rather than a
/// clear, so repeated traversals from a hot node cost nothing between
/// runs.
#[derive(Default)]
pub(crate) struct VisitSet {
    stamp: Vec<u32>,
    version: u32,
}

impl VisitSet {
    /// Begin a traversal. Wrapping the counter would alias stale stamps
    /// with the new generation, so the array is zeroed on the
    /// (astronomically rare) wrap and numbering restarts.
    pub(crate) fn begin(&mut self) {
        self.version = match self.version.checked_add(1) {
            Some(v) => v,
            None => {
                self.stamp.fill(0);
                1
            }
        };
    }

    /// Mark `id` visited. Returns true if it was not already.
    #[inline]
    pub(crate) fn visit(&mut self, id: Id) -> bool {
        let i = id.index() as usize;
        if i >= self.stamp.len() {
            self.stamp.resize(i + 1, 0);
        }
        let slot = &mut self.stamp[i];
        let fresh = *slot != self.version;
        *slot = self.version;
        fresh
    }
}

/// Reused by the cycle check and by the no-reverse scan, so neither
/// allocates on the hot path.
#[derive(Default)]
struct Scratch {
    stack: Vec<Id>,
    seen: VisitSet,
    found: Vec<Id>,
}

pub struct Directed {
    forward: Index,
    reverse: Option<Index>,
    acyclic: bool,
    scratch: Scratch,
}

impl Directed {
    pub(crate) fn new(unique_source: bool, unique_target: bool, acyclic: bool, reverse: bool, meta: &TypeMeta) -> Self {
        Self {
            forward: Index::new(unique_source, meta),
            reverse: (reverse || unique_target).then(|| Index::new(unique_target, <()>::META)),
            scratch: Scratch::default(),
            acyclic,
        }
    }

    #[inline(always)]
    pub(crate) fn has_reverse(&self) -> bool {
        self.reverse.is_some()
    }

    #[inline(always)]
    pub(crate) fn outgoing(&self, source: Id) -> Option<Edges<'_>> {
        self.forward.get(source)
    }

    /// Sources pointing at `target`.
    ///
    /// # Panics
    /// Panics if called on a directed relationship without
    /// a reverse index. This is because without a reverse
    /// index, incoming edges requires a source scan which
    /// is too expensive for regular usage.
    ///
    /// The relationship must explicitly declare a reverse
    /// index, with the tradeoff of more memory usage.
    #[inline(always)]
    pub(crate) fn incoming(&self, target: Id) -> Option<Edges<'_>> {
        self.reverse
            .as_ref()
            .expect("INTERNAL ERROR: `incoming` without a reverse index")
            .get(target)
    }

    #[inline(always)]
    pub(crate) fn contains(&self, source: Id, target: Id) -> bool {
        self.forward.contains(source, target)
    }

    #[inline(always)]
    pub(crate) fn has_outgoing(&self, source: Id) -> bool {
        !self.forward.is_empty(source)
    }

    #[inline(always)]
    pub(crate) fn has_incoming(&self, target: Id) -> bool {
        !self
            .reverse
            .as_ref()
            .expect("INTERNAL ERROR: `incoming` check without a reverse index")
            .is_empty(target)
    }

    /// Create `source -> target`.
    ///
    /// Uniqueness replaces rather than rejects: a `unique_source`
    /// relation drops the source's existing edge, a `unique_target` one
    /// drops the target's. Both happen before insertion, so the
    /// insertion path itself has no cardinality cases.
    ///
    /// # Safety
    /// `payload` is a value of the declared relationship payload type.
    pub(crate) unsafe fn relate<T>(&mut self, source: Id, target: Id, payload: T) -> Result<(), RelateError> {
        if let Some(data) = unsafe { self.forward.payload_mut(source, target) } {
            let _ = std::mem::replace(data, payload);
            return Ok(());
        }

        if self.acyclic && self.reaches(target, source) {
            return Err(RelateError::CycleOnAcyclic(source, target));
        }

        // Displacement runs before insertion, so neither eviction can
        // see the edge being created and remove it again.
        self.displace_unique_source(source);
        self.displace_unique_target(target);

        unsafe {
            // SAFETY: caller guarantees `T`.
            self.forward.add(source, target, payload);
            // SAFETY: reverse index has payload type `()`.
            self.reverse.as_mut().map(|s| s.add(target, source, ()));
        }

        Ok(())
    }

    pub(crate) fn unrelate(&mut self, source: Id, target: Id) {
        if self.forward.remove(source, target)
            && let Some(rev) = &mut self.reverse
        {
            rev.remove(target, source);
        }
    }

    /// Drop `source`'s existing edge. The primary is `ToOne` here, so
    /// this is one read.
    fn displace_unique_source(&mut self, source: Id) {
        if let Some(old) = self.forward.get_unique(source) {
            self.unrelate(source, old);
        }
    }

    /// Drop the edge currently pointing at `target`. `unique_target`
    /// guarantees the reverse index exists and is `ToOne`.
    fn displace_unique_target(&mut self, target: Id) {
        if let Some(rev) = &mut self.reverse
            && let Some(source) = rev.get_unique(target)
        {
            self.unrelate(source, target);
        }
    }

    /// Despawn cascade: every edge touching `id`, in either role.
    pub(crate) fn purge(&mut self, id: Id) {
        // As a source: the forward yields the far ends directly.
        // Note: match done here to avoid a branch each iteration.
        let targets = self.forward.remove_all(id);
        if let Some(rev) = &mut self.reverse {
            for target in targets {
                rev.remove(target, id);
            }
        }

        // As a target: the secondary answers in one call. Without one,
        // finding `id` among the primary's targets is a full scan —
        // the documented cost of declaring a directed relation with no
        // reverse index.
        let mut sources = std::mem::take(&mut self.scratch.found);
        sources.clear();

        match &mut self.reverse {
            Some(rev) => sources.extend(rev.remove_all(id)),
            None => self.forward.find_sources(id, &mut sources),
        }

        for source in &sources {
            self.forward.remove(*source, id);
        }

        self.scratch.found = sources;
    }

    /// Is `goal` reachable from `from` along the relation?
    ///
    /// The graph is acyclic before every insert, so a new edge
    /// `source -> target` closes a loop exactly when `source` is
    /// already reachable from `target`.
    fn reaches(&mut self, from: Id, goal: Id) -> bool {
        match &self.forward.target {
            // `unique_source`: one successor per node, so the path is
            // linear and cannot revisit — a repeat would already be a
            // cycle, which the invariant forbids. O(depth), no scratch.
            Target::One(e) => {
                let mut cur = from;
                loop {
                    if cur == goal {
                        return true;
                    }
                    match self.forward.sparse.get(cur) {
                        Some(&next) => cur = e.target(next),
                        None => return false,
                    }
                }
            }
            // Many successors: DFS over the subgraph reachable from
            // `from` — the descendant set, not the whole relation.
            //
            // `seen` is required rather than an optimisation: a DAG can
            // reach one node by several paths, and a diamond without it
            // re-traverses exponentially.
            Target::Many(e) => {
                let Scratch { stack, seen, .. } = &mut self.scratch;
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
                    if let Some(&slot) = self.forward.sparse.get(cur) {
                        stack.extend_from_slice(e[slot].targets());
                    }
                }
                false
            }
        }
    }
}
