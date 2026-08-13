use crate::{
    Id, TypeMeta,
    relation::storage::{Edges, Index, RelateError},
};

/// Undirected edges, stored from both ends.
///
/// A symmetric edge `{a, b}` is one whose endpoints are
/// interchangeable. Each edge is inserted twice, once from each end.
///
/// Payload is keyed by the canonical edge `(min(a, b), max(a, b))`
/// by the canonical half.
///
/// Acyclicity is unrepresentable in symmetry. `{a, b}` is already a 2-cycle,
/// and there is no direction to reverse.
pub(crate) struct Symmetry {
    edges: Index,
}

impl Symmetry {
    pub(super) fn new(unique: bool, meta: &TypeMeta) -> Self {
        Self { edges: Index::new(unique, meta) }
    }

    /// Every entity sharing an edge with `id`.
    #[inline(always)]
    pub(super) fn neighbors(&self, id: Id) -> Option<Edges<'_>> {
        self.edges.get(id)
    }

    /// Both directions are stored, so either argument
    /// order finds the edge.
    #[inline(always)]
    pub(super) fn contains(&self, a: Id, b: Id) -> bool {
        debug_assert!(self.edges.contains(b, a));
        self.edges.contains(a, b)
    }

    #[inline(always)]
    pub(super) fn has_edges(&self, id: Id) -> bool {
        !self.edges.is_empty(id)
    }

    /// Create `{a, b}`.
    ///
    /// # Safety
    /// `payload` is a value if the declared relationship type `T`.
    pub(super) unsafe fn relate<T>(&mut self, a: Id, b: Id, payload: T) -> Result<(), RelateError> {
        // A self-edge has no meaning here. {a, a} would make an
        // entity its own partner and break exclusive displacement.
        if a == b {
            return Err(RelateError::SelfEdgeOnSymmetric(a));
        }

        if let Some(data) = unsafe { self.edges.payload_mut(a, b) } {
            let _ = std::mem::replace(data, payload);
            return Ok(());
        }

        // Exclusive: the new pairing evicts whatever each end had.
        self.displace_exclusive(a);
        self.displace_exclusive(b);

        unsafe {
            self.edges.add(a, b, payload);
            // todo: self.edges.add(b, a, second);
        }

        Ok(())
    }

    /// Remove `{a, b}`, if present. This removes the corresponding
    /// `{b, a}` relationship to maintain symmetry.
    pub(super) fn unrelate(&mut self, a: Id, b: Id) {
        if self.edges.remove(a, b) {
            self.edges.remove(b, a);
        }
        // todo: remove payload data
    }

    /// Drop `id`'s current exclusive pairing so a new one can take its place.
    fn displace_exclusive(&mut self, id: Id) {
        if let Some(p) = self.edges.get_unique(id) {
            self.unrelate(id, p);
        }
    }

    /// Despawn cascade: every edge touching `id`.
    ///
    /// Both halves live in one index, so the far ends come back from a
    /// single `remove_all` and each needs only its mirror cleared.
    pub(super) fn purge(&mut self, id: Id) {
        for far in self.edges.remove_all(id) {
            self.edges.remove(far, id);
        }
    }
}
