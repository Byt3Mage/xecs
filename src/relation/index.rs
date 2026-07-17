use std::{ptr::NonNull, rc::Rc};

use crate::{Id, TypeMeta, relation::RelationProps, storage::blob::Blob};

pub(crate) const NONE: u32 = u32::MAX;

#[inline(always)]
const fn ix(id: Id) -> usize {
    id.index() as usize
}

pub(crate) enum EdgeOp {
    Set {
        source: Id,
        target: Id,
        data: Option<NonNull<u8>>,
    },
    Remove {
        source: Id,
        target: Id,
    },
}

pub(crate) struct EdgeData {
    data: Blob,
    len: usize,
    cap: usize,
}

impl EdgeData {
    fn new(meta: Rc<TypeMeta>) -> Self {
        Self { data: Blob::new(meta), len: 0, cap: 0 }
    }
}

pub(crate) struct SparseEdges {
    sparse: Vec<u32>,
    pub(crate) source: Vec<Id>,
    pub(crate) target: Vec<Id>,
}

impl SparseEdges {
    fn new() -> Self {
        Self { sparse: vec![], source: vec![], target: vec![] }
    }

    #[inline(always)]
    pub(crate) fn slot_of(&self, src: Id) -> Option<u32> {
        self.sparse.get(ix(src)).filter(|&&s| s != NONE).copied()
    }

    #[inline(always)]
    pub(crate) fn target(&self, src: Id) -> Option<Id> {
        self.slot_of(src).map(|s| self.target[s as usize])
    }

    /// Insert or retarget. Returns (slot, retargeted).
    fn set(&mut self, source: Id, target: Id) -> (u32, bool) {
        let idx = ix(source);
        if idx >= self.sparse.len() {
            self.sparse.resize(idx + 1, NONE);
        }

        match self.sparse[idx] {
            NONE => {
                let slot = self.source.len() as u32;
                self.source.push(source);
                self.target.push(target);
                self.sparse[idx] = slot;
                (slot, false)
            }
            slot => {
                self.target[slot as usize] = target;
                (slot, true)
            }
        }
    }

    /// Remove source's edge. Returns (freed slot, slot that was moved
    /// into it — the previous last — if any).
    fn remove(&mut self, source: Id) -> Option<(u32, Option<u32>)> {
        let slot = self.slot_of(source)?;
        let last = (self.source.len() - 1) as u32;

        self.sparse[ix(source)] = NONE;
        self.source.swap_remove(slot as usize);
        self.target.swap_remove(slot as usize);

        let mut prev_last = None;
        if slot != last {
            self.sparse[ix(self.source[slot as usize])] = slot;
            prev_last = Some(last);
        }

        Some((slot, prev_last))
    }
}

pub(crate) struct DirectedCsr {
    node_of: Vec<u32>,
    offsets: Vec<u32>,
    pub(crate) source: Vec<Id>,
    pub(crate) target: Vec<Id>,
}

impl DirectedCsr {
    fn new() -> Self {
        Self {
            node_of: vec![],
            offsets: vec![0],
            source: vec![],
            target: vec![],
        }
    }

    pub fn slots_of(&self, src: Id) -> std::ops::Range<usize> {
        self.node_of
            .get(ix(src))
            .and_then(|&n| (n != NONE).then_some(n as usize))
            .map_or(0..0, |n| self.offsets[n] as usize..self.offsets[n + 1] as usize)
    }

    /// Rebuild from (source, target, payload_move) triples. `payload_move`
    /// = old payload slot to carry over, or NONE for fresh edges. Sorted
    /// by source index (stable — preserves insertion order within a run).
    /// Returns (old_slot -> new_slot) moves for payload permutation and
    /// the fresh (new_slot, op_index) list for payload writes.
    fn rebuild(&mut self, mut edges: Vec<(Id, Id, u32)>) -> Vec<(u32, u32)> {
        edges.sort_by_key(|&(s, _, _)| s.index());

        let mut moves = Vec::new();
        self.source.clear();
        self.target.clear();
        self.node_of.clear();
        self.offsets.clear();
        self.offsets.push(0);

        let mut current: Option<Id> = None;
        for (new_slot, &(s, t, old)) in edges.iter().enumerate() {
            if current != Some(s) {
                // Close the previous run, open a new node.
                if current.is_some() {
                    self.offsets.push(new_slot as u32);
                }
                let node = (self.offsets.len() - 1) as u32;
                if ix(s) >= self.node_of.len() {
                    self.node_of.resize(ix(s) + 1, NONE);
                }
                self.node_of[ix(s)] = node;
                current = Some(s);
            }
            self.source.push(s);
            self.target.push(t);
            if old != NONE {
                moves.push((old, new_slot as u32));
            }
        }
        self.offsets.push(edges.len() as u32);
        moves
    }
}

/// Undirected adjacency. Each logical edge occupies ONE slot (payload,
/// stored elsewhere, is per-slot) but TWO entries — one in each endpoint's
/// run. `entry_slot` maps entries back to their logical edge.
///
/// Invariants:
/// - `offsets.len() == node_count + 1`; monotone; `*offsets.last() == neighbor.len()`.
/// - `neighbor.len() == entry_slot.len() == 2 * edge_count`.
/// - Every slot value in `entry_slot` appears exactly twice, and the two
///   entries are mirror images: if entry a in n0's run has slot s and
///   neighbor n1, then some entry in n1's run has slot s and neighbor n0.
/// - Self-edges are forbidden at batch time (they would collapse the pair).
/// - Rebuilt only at sync via apply_batch.
pub(crate) struct UndirectedCsr {
    node_of: Vec<u32>,
    offsets: Vec<u32>,
    pub(crate) neighbor: Vec<Id>,    // entry -> the other endpoint
    pub(crate) entry_slot: Vec<u32>, // entry -> logical edge slot (payload index)
}

impl UndirectedCsr {
    fn new() -> Self {
        Self {
            node_of: vec![],
            offsets: vec![0],
            neighbor: vec![],
            entry_slot: vec![],
        }
    }

    pub fn entries_of(&self, id: Id) -> std::ops::Range<usize> {
        self.node_of
            .get(ix(id))
            .and_then(|&n| (n != NONE).then_some(n as usize))
            .map_or(0..0, |n| self.offsets[n] as usize..self.offsets[n + 1] as usize)
    }

    /// Rebuild from canonical logical edges (a, b, payload_move), a/b in
    /// stored order. Slot i = edge i; entries mirrored under both ends.
    fn rebuild(&mut self, edges: &[(Id, Id, u32)]) -> Vec<(u32, u32)> {
        // (owner, neighbor, slot) — two entries per edge.
        let mut entries = Vec::with_capacity(edges.len() * 2);
        let mut moves = Vec::new();
        for (slot, &(a, b, old)) in edges.iter().enumerate() {
            entries.push((a, b, slot as u32));
            entries.push((b, a, slot as u32));
            if old != NONE {
                moves.push((old, slot as u32));
            }
        }
        entries.sort_by_key(|&(o, _, _)| o.index());

        self.neighbor.clear();
        self.entry_slot.clear();
        self.node_of.clear();
        self.offsets.clear();
        self.offsets.push(0);

        let mut current: Option<Id> = None;
        for (i, &(o, n, s)) in entries.iter().enumerate() {
            if current != Some(o) {
                if current.is_some() {
                    self.offsets.push(i as u32);
                }
                let node = (self.offsets.len() - 1) as u32;
                if ix(o) >= self.node_of.len() {
                    self.node_of.resize(ix(o) + 1, NONE);
                }
                self.node_of[ix(o)] = node;
                current = Some(o);
            }
            self.neighbor.push(n);
            self.entry_slot.push(s);
        }
        self.offsets.push(entries.len() as u32);
        moves
    }
}

pub(crate) enum Forward {
    Sparse(SparseEdges),
    Csr(DirectedCsr),
    Undirected(UndirectedCsr),
}

pub(crate) enum Reverse {
    None,
    Sparse(Vec<u32>),
    Csr {
        node_of: Vec<u32>,
        offsets: Vec<u32>,
        slots: Vec<u32>,
    },
}

#[derive(Default)]
pub(crate) struct TreeExtras {
    depth: Vec<u16>, // per dense slot
    topo: Vec<Id>,   // parents-before-children, lazy
    topo_dirty: bool,
}

pub(crate) struct RelationIndex {
    pub(crate) forward: Forward,
    pub(crate) reverse: Reverse,
    pub(crate) tree: Option<TreeExtras>,
    pub(crate) edge_data: Option<EdgeData>,
}

impl RelationIndex {
    pub(crate) fn select(props: &RelationProps, edge_meta: Option<Rc<TypeMeta>>) -> Self {
        let forward = match (props.symmetric, props.unique_source) {
            (true, _) => Forward::Undirected(UndirectedCsr::new()),
            (_, true) => Forward::Sparse(SparseEdges::new()),
            (_, false) => Forward::Csr(DirectedCsr::new()),
        };

        let reverse = match (props.symmetric || !props.indexed_reverse, props.unique_target) {
            (true, _) => Reverse::None,
            (_, true) => Reverse::Sparse(vec![]),
            (_, false) => Reverse::Csr { offsets: vec![], node_of: vec![], slots: vec![] },
        };

        Self {
            tree: (props.unique_source && props.acyclic).then(TreeExtras::default),
            edge_data: edge_meta.map(EdgeData::new),
            forward,
            reverse,
        }
    }

    pub fn has_forward(&self, id: Id) -> bool {
        match &self.forward {
            Forward::Sparse(s) => s.slot_of(id).is_some(),
            Forward::Csr(c) => !c.slots_of(id).is_empty(),
            Forward::Undirected(u) => !u.entries_of(id).is_empty(),
        }
    }

    pub fn has_reverse(&self, id: Id) -> bool {
        match &self.reverse {
            Reverse::None => unreachable!("reverse probe without index: query lowering bug"),
            Reverse::Sparse(s) => s.get(ix(id)).is_some_and(|&s| s != NONE),
            Reverse::Csr { node_of, offsets, .. } => node_of
                .get(ix(id))
                .and_then(|&n| (n < u32::MAX).then_some(n as usize))
                .is_some_and(|n| offsets[n] < offsets[n + 1]),
        }
    }

    pub fn contains(&self, src: Id, tgt: Id) -> bool {
        match &self.forward {
            Forward::Sparse(s) => s.target(src).is_some_and(|t| t == tgt),
            Forward::Csr(c) => c.target[c.slots_of(src)].contains(&tgt),
            Forward::Undirected(u) => u.neighbor[u.entries_of(src)].contains(&tgt),
        }
    }
}
