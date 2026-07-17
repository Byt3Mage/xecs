use std::cmp::Ordering;

use crate::{
    Ecs, Id,
    query::{
        QueryCtx,
        error::LowerError,
        iter::{Bindings, Params},
        logical::{LogicalPlan, Scope, ScopeId},
    },
    relation::{
        RelationId,
        index::{Forward, NONE, Reverse},
    },
    table_index::TableId,
};

/// A logical plan resolved against a concrete world. Flat, parallel to
/// the logical plan's scopes/joins by index. Invalidated (re-lowered)
/// when `resolved_at != ecs.generation`.
pub struct PhysicalPlan {
    pub(crate) resolved_at: u64,

    /// Per scope, same indices as logical.
    /// - `scopes[0]` = driver: its matched tables are iterated
    /// - Tables in other scopes are probed via [PhysicalScope::resolve].
    pub(crate) scopes: Box<[PhysicalScope]>,

    /// Per join, same indices as logical. Carries everything execution
    /// needs.
    pub(crate) joins: Box<[PhysicalJoin]>,
}

pub(crate) struct PhysicalScope {
    /// Tables satisfying the scope's component constraints, ordered
    /// by [TableId], with per-select column resolution.
    pub(crate) tables: Box<[MatchedTable]>,

    /// This scope's relationship check. Run when the scope binds:
    /// - driver checks per driver row
    /// - destination checks during fan advancement.
    pub(crate) checks: Box<[Check]>,

    /// Guard list: earlier-bound scopes this one must differ from.
    /// Fired once, at bind time: bound[earlier] == candidate -> skip.
    /// Validator emits each RowGuard{a,b} into the LATER scope's list,
    /// so every guard fires exactly once per binding.
    pub(crate) guards: Box<[ScopeId]>,
}

impl PhysicalScope {
    /// Membership + column routing in one step. Binary search over the
    /// sorted matched list; join-path callers wrap this in a last-table
    /// cache so cost is per table-run, not per target.
    #[inline]
    pub(crate) fn resolve(&self, id: TableId) -> Option<&MatchedTable> {
        let tables = &self.tables;
        tables.binary_search_by_key(&id, |t| t.id).ok().map(|i| &tables[i])
    }

    #[inline]
    pub fn bind<'p>(&'p self, scope: ScopeId, id: Id, ctx: &QueryCtx<'_>) -> Option<(Id, u32, &'p MatchedTable)> {
        let record = ctx.ecs.ids.get(id).expect("edge to dead id: purge broken");
        let matched = self.resolve(record.table)?;

        // Run relationship checks
        if !self.checks.iter().all(|c| c.eval(id, ctx)) {
            return None;
        }

        // Run guard checks
        if self.guards.iter().any(|&g| ctx.binds.get(g) == id) {
            return None;
        }

        ctx.binds.set(scope, id);

        Some((id, record.row, matched))
    }
}

pub struct MatchedTable {
    pub(crate) id: TableId,
    pub(crate) columns: Box<[usize]>,
}

/// Per-row filter. Direction, target kind, negation,
/// and relationship shape are all resolved into the variant; execution
/// dispatches once on this enum and touches storage directly.
#[derive(Copy, Clone)]
pub(crate) enum Check {
    /// (Rel, _): any outgoing edge exists.
    AnyForward { rel: RelationId, neg: bool },
    /// (<Rel, _): any incoming edge exists (reverse index, capability-
    /// checked at lowering).
    AnyReverse { rel: RelationId, neg: bool },
    /// Edge scope_entity -> X exists. Reversed pinned checks were
    /// flipped at lowering (contains(me, X, rev) == contains(X, me, fwd)),
    /// so execution has exactly one containment orientation.
    EdgeTo { rel: RelationId, tgt: IdSource, neg: bool },
    /// Edge X -> scope_entity exists. The flipped form of [CheckMode::EdgeTo].
    EdgeFrom { rel: RelationId, tgt: IdSource, neg: bool },
}

impl Check {
    #[inline]
    pub(crate) fn eval(&self, id: Id, QueryCtx { ecs, binds, params, .. }: &QueryCtx) -> bool {
        let rels = &ecs.relations;
        match *self {
            Check::AnyForward { rel, neg } => rels.index(rel).has_forward(id) != neg,
            Check::AnyReverse { rel, neg } => rels.index(rel).has_reverse(id) != neg,
            Check::EdgeTo { rel, tgt, neg } => rels.index(rel).contains(id, tgt.resolve(binds, params)) != neg,
            Check::EdgeFrom { rel, tgt, neg } => rels.index(rel).contains(tgt.resolve(binds, params), id) != neg,
        }
    }
}
/// Where a comparison entity comes from at row time. Shared by probes
/// and joins.
#[derive(Copy, Clone)]
pub(crate) enum IdSource {
    /// #id: fixed at build.
    Fixed(Id),
    /// $n: supplied at dispatch, resolved once per execution.
    Param(usize),
    /// label: read from bindings when the probe runs.
    Scope(ScopeId),
}

impl IdSource {
    #[inline(always)]
    pub(crate) fn resolve(&self, binds: &Bindings, params: &Params) -> Id {
        match *self {
            IdSource::Fixed(i) => i,
            IdSource::Param(n) => params.get(n),
            IdSource::Scope(s) => binds.get(s),
        }
    }
}

/// One join, fully lowered.
pub(crate) struct PhysicalJoin {
    pub(crate) relation: RelationId,
    pub(crate) reversed: bool,
    pub(crate) optional: bool,
    pub(crate) pinned_id: Option<IdSource>,
    pub(crate) from: ScopeId,
    pub(crate) dest: ScopeId,
}

impl PhysicalJoin {
    pub(crate) fn fan<'w>(&self, from: Id, bindings: &Bindings, params: &Params, ecs: &'w Ecs) -> Fan<'w> {
        let index = ecs.relations.index(self.relation);

        if let Some(pin) = self.pinned_id {
            let pinned = pin.resolve(bindings, params);
            let exists = if self.reversed { index.contains(pinned, from) } else { index.contains(from, pinned) };
            return Fan::One(exists.then_some(pinned));
        }

        if !self.reversed {
            match &index.forward {
                Forward::Sparse(s) => Fan::One(s.target(from)),
                Forward::Csr(c) => Fan::Slice(c.target[c.slots_of(from)].iter()),
                Forward::Undirected(u) => Fan::Slice(u.neighbor[u.entries_of(from)].iter()),
            }
        } else {
            // Reversed enumeration: walk the derived reverse index.
            // Candidates are edge SOURCES, read through forward's dense
            // source column in both cases.
            let source: &[Id] = match &index.forward {
                Forward::Sparse(s) => &s.source,
                Forward::Csr(c) => &c.source,
                Forward::Undirected(_) => unreachable!("reversed fan on symmetric relation: lowering bug"),
            };

            match &index.reverse {
                Reverse::None => unreachable!("reversed fan without index: lowering bug"),
                Reverse::Sparse(s) => {
                    let t = s
                        .get(from.index() as usize)
                        .and_then(|&i| (i != NONE).then_some(i as usize))
                        .map(|i| source[i]);
                    Fan::One(t)
                }
                Reverse::Csr { node_of, offsets, slots } => {
                    let r = node_of
                        .get(from.index() as usize)
                        .and_then(|&n| (n != NONE).then_some(n as usize))
                        .map_or(0..0, |n| offsets[n] as usize..offsets[n + 1] as usize);
                    Fan::Indirect { slots: slots[r].iter(), source }
                }
            }
        }
    }
}

/// One join's edge run for a single `from` entity. Pure candidates:
/// no gating, no columns. Empty fans are values (One(None), empty
/// slice), not errors — "no edges" is a walk of zero.
pub(crate) enum Fan<'w> {
    /// Sparse shapes and pinned joins: zero or one candidate.
    One(Option<Id>),
    /// Forward CSR / undirected: dest entities are contiguous.
    Slice(std::slice::Iter<'w, Id>),
    /// Reverse CSR: contiguous slot run, dest = source[slot].
    /// One indirection inherent to the derived index.
    Indirect {
        slots: std::slice::Iter<'w, u32>,
        source: &'w [Id],
    },
}

impl Iterator for Fan<'_> {
    type Item = Id;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Fan::One(v) => v.take(),
            Fan::Slice(it) => it.next().copied(),
            Fan::Indirect { slots, source } => slots.next().map(|&s| source[s as usize]),
        }
    }
}

fn match_scope(Scope { access, with, without, .. }: &Scope, ecs: &Ecs) -> Box<[MatchedTable]> {
    if access.is_empty() && with.is_empty() {
        // Every table matches, so we remove tables
        // that have 'without' components.
        return ecs
            .tables
            .iter()
            .enumerate()
            .filter(|(_, t)| !without.iter().any(|c| t.sig.has_id(c)))
            .map(|(i, _)| MatchedTable { id: TableId(i as u32), columns: Box::new([]) })
            .collect();
    }

    let smallest = with
        .iter()
        .chain(access.iter().filter_map(|a| (!a.optional).then_some(&a.id)))
        .min_by_key(|&&c| ecs.components[c].tables.len())
        .map(|&c| &ecs.components[c].tables)
        .unwrap(); // required components must be non-empty at this point

    smallest
        .iter()
        .filter_map(|&id| {
            let sig = &ecs.tables[id].sig;

            if !with.iter().all(|c| sig.has_id(c)) || without.iter().any(|c| sig.has_id(c)) {
                return None;
            }

            access
                .iter()
                .map(|a| sig.find_id(&a.id).or_else(|| a.optional.then_some(usize::MAX)))
                .collect::<Option<_>>()
                .map(|columns| MatchedTable { id, columns })
        })
        .collect()
}

fn check_capabilities(logical: &LogicalPlan, ecs: &Ecs) -> Result<(), LowerError> {
    let joins = logical.joins.iter().filter_map(|j| j.relation.as_reversed());
    let scopes = logical.scopes.iter();
    let checks = scopes.flat_map(|s| s.rel_check.iter().filter_map(|c| c.relation.as_reversed()));

    for relation in joins.chain(checks) {
        let props = ecs.relations.get(relation.id).props;

        // Symmetric edges must have no direction.
        if props.symmetric {
            return Err(LowerError::ReversedSymmetric(relation.id));
        }

        // Reversed enumeration walks the reverse index.
        // So reversed relationships without a pinned target must have one.
        if !props.indexed_reverse && relation.target.is_any() {
            return Err(LowerError::NoReverseIndex(relation.id));
        }
    }

    Ok(())
}

/// Sorted-slice overlap test. scope_tables lists are ascending by
/// construction: the inverted index appends TableIds in creation order,
/// and match_scope filters without reordering.
fn cleared_by_nonoverlap(a: &[MatchedTable], b: &[MatchedTable]) -> bool {
    let (mut ai, mut bi) = (0, 0);
    while ai < a.len() && bi < b.len() {
        match a[ai].id.cmp(&b[bi].id) {
            Ordering::Equal => return false,
            Ordering::Less => ai += 1,
            Ordering::Greater => bi += 1,
        }
    }
    true
}

/// Does some single join directly connect scopes (x, y) over an acyclic
/// relationship? Acyclicity forbids self-edges, so from and dest of one
/// hop can't be the same entity, regardless of pinning or reversal.
fn cleared_by_acyclicity(x: ScopeId, y: ScopeId, logical: &LogicalPlan, ecs: &Ecs) -> bool {
    logical.joins.iter().any(|j| {
        let connected = (j.from == x && j.dest == y) || (j.from == y && j.dest == x);
        connected && ecs.relations.get(j.relation.id).props.acyclic
    })
}

/// Minimal union-find over scope indices. Path-halving find; scopes are
/// few enough that rank tracking is pointless.
struct ScopeUnion(Box<[usize]>);

impl ScopeUnion {
    fn new(n: usize) -> Self {
        Self((0..n).collect())
    }

    fn resolve(&mut self, mut x: usize) -> usize {
        while self.0[x] != x {
            self.0[x] = self.0[self.0[x]]; // path halving
            x = self.0[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.resolve(a), self.resolve(b));
        if a != b {
            self.0[a] = b;
        }
    }
}
