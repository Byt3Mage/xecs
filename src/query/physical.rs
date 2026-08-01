use crate::{
    Id,
    query::{
        context::{Binds, QueryCtx},
        logical::ScopeId,
    },
    relation::{
        RelationId,
        index::{Edges, Topology},
    },
    table_index::TableId,
};

/// A logical plan resolved against a concrete world. Flat, parallel to
/// the logical plan's scopes/joins by index. Invalidated (re-lowered)
/// when `resolved_at != ecs.generation`.
#[derive(Debug, Clone)]
pub struct PhysicalPlan {
    pub(crate) resolved_at: u32,

    /// Per scope, same indices as logical.
    /// - `scopes[0]` = driver; its matched tables are iterated
    /// - Tables in other scopes are probed via [PhysicalScope::resolve].
    pub(crate) scopes: Box<[PhysicalScope]>,

    /// Per join, same indices as logical.
    /// Carries everything join execution needs.
    pub(crate) follows: Box<[PhysicalFollow]>,
}

#[derive(Debug, Clone)]
pub(crate) struct PhysicalScope {
    /// Tables satisfying the scope's component constraints, ordered
    /// by [TableId], with per-select column resolution.
    pub(crate) matched_tables: Box<[MatchedTable]>,

    /// This scope's relationship check. Run when the scope binds:
    /// - driver checks per driver row
    /// - destination checks during fan advancement.
    pub(crate) relation_checks: Box<[Check]>,

    /// Guard list: earlier-bound scopes this one must differ from.
    /// Fired once, at bind time: bound[earlier] == candidate -> skip.
    /// Validator emits each RowGuard{a,b} into the LATER scope's list,
    /// so every guard fires exactly once per binding.
    pub(crate) scope_guards: Box<[ScopeId]>,
}

impl PhysicalScope {
    /// Membership + column routing in one step. Binary search over the
    /// sorted matched list; join-path callers wrap this in a last-table
    /// cache so cost is per table-run, not per target.
    #[inline]
    pub(crate) fn resolve(&self, id: TableId) -> Option<&MatchedTable> {
        let tables = &self.matched_tables;
        tables.binary_search_by_key(&id, |t| t.id).ok().map(|i| &tables[i])
    }

    #[inline(always)]
    pub(crate) fn passes_relation_checks(&self, id: Id, ctx: &QueryCtx<'_>) -> bool {
        self.relation_checks.iter().all(|c| c.eval(id, ctx))
    }

    #[inline(always)]
    pub(crate) fn passes_scope_guards(&self, id: Id, ctx: &QueryCtx<'_>) -> bool {
        self.scope_guards.iter().all(|&g| ctx.binds.get(g) != id)
    }
}

#[derive(Debug, Clone)]
pub struct MatchedTable {
    pub(crate) id: TableId,
    pub(crate) columns: Box<[usize]>,
}

/// Per-row filter. Direction, target kind, negation,
/// and relationship shape are all resolved into the variant; execution
/// dispatches once on this enum and touches storage directly.
#[derive(Debug, Copy, Clone)]
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
            Check::AnyForward { rel, neg } => rels.index(rel).has_outgoing(id) != neg,
            Check::AnyReverse { rel, neg } => rels.index(rel).has_incoming(id) != neg,
            Check::EdgeTo { rel, tgt, neg } => rels.index(rel).contains(id, tgt.resolve(binds, params)) != neg,
            Check::EdgeFrom { rel, tgt, neg } => rels.index(rel).contains(tgt.resolve(binds, params), id) != neg,
        }
    }
}
/// Where a comparison entity comes from at row time. Shared by probes
/// and joins.
#[derive(Debug, Copy, Clone)]
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
    pub(crate) fn resolve(&self, binds: &Binds, params: &[Id]) -> Id {
        match *self {
            IdSource::Fixed(i) => i,
            IdSource::Param(n) => params[n],
            IdSource::Scope(s) => binds.get(s),
        }
    }
}

/// One join, fully lowered.
#[derive(Debug, Clone)]
pub(crate) struct PhysicalFollow {
    pub(crate) relation: RelationId,
    pub(crate) reversed: bool,
    pub(crate) pinned_id: Option<IdSource>,
    pub(crate) scope: ScopeId,
}

impl PhysicalFollow {
    pub(crate) fn fan<'w>(&self, id: Id, ctx: &QueryCtx<'w>) -> Fan<'w> {
        let index = ctx.ecs.relations.index(self.relation);

        if let Some(pin) = self.pinned_id {
            let pin = pin.resolve(&ctx.binds, ctx.params);
            let has = if self.reversed { index.contains(pin, id) } else { index.contains(id, pin) };
            return Fan::One(has.then_some(pin));
        }

        if index.props().topology == Topology::Symmetric {
            return match (index.outgoing(id), index.incoming(id)) {
                (Edges::One(a), Edges::One(b)) => Fan::One(a.or(b)),
                (Edges::Many(a), Edges::Many(b)) => Fan::Both(a.iter(), b.iter()),
                _ => unreachable!("symmetric halves share arity: select bug"),
            };
        }

        Fan::from_edges(if self.reversed { index.incoming(id) } else { index.outgoing(id) })
    }
}

/// One join's edge run for a single `from` entity.
/// Empty fans are values , not errors.
pub(crate) enum Fan<'w> {
    One(Option<Id>),
    Many(std::slice::Iter<'w, Id>),
    Both(std::slice::Iter<'w, Id>, std::slice::Iter<'w, Id>),
}

impl<'w> Fan<'w> {
    #[inline(always)]
    fn from_edges(e: Edges<'w>) -> Self {
        match e {
            Edges::One(id) => Self::One(id),
            Edges::Many(ids) => Self::Many(ids.iter()),
        }
    }
}

impl Iterator for Fan<'_> {
    type Item = Id;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Fan::One(v) => v.take(),
            Fan::Many(it) => it.next().copied(),
            Fan::Both(a, b) => a.next().or_else(|| b.next()).copied(),
        }
    }
}
