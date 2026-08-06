use std::rc::Rc;

use crate::{
    Id,
    query::{
        context::QueryCtx,
        logical::{FollowId, IdSource, ScopeId},
    },
    relation::{RelationId, index::Edges},
    table_index::TableId,
};

/// A logical plan resolved against a concrete world. Flat, parallel to
/// the logical plan's scopes/joins by index. Invalidated (re-lowered)
/// when `resolved_at != ecs.generation`.
#[derive(Debug, Clone)]
pub struct PhysicalPlan {
    pub(crate) resolved_at: u32,

    /// Tables satisfying the scope's component constraints, ordered
    /// by [TableId], with per-select column resolution.
    pub(crate) tables: Rc<[MatchedTable]>,

    pub(crate) scopes: Rc<[PhysicalScope]>,

    pub(crate) follows: Rc<[PhysicalFollow]>,

    pub(crate) all_follows_injective: bool,
}

#[derive(Debug, Clone)]
pub struct MatchedTable {
    pub(crate) id: TableId,
    pub(crate) columns: Box<[usize]>,
}

#[derive(Debug, Clone)]
pub(crate) struct PhysicalScope {
    /// Tables satisfying the scope's component constraints, ordered
    /// by [TableId], with per-select column resolution.
    pub(crate) filter: Option<Box<[TableId]>>,

    /// This scope's relationship check. Run when the scope binds:
    /// - driver checks per driver row
    /// - destination checks during fan advancement.
    pub(crate) checks: Box<[PhysicalCheck]>,

    pub(crate) follows: Box<[FollowId]>,
}

impl PhysicalScope {
    #[inline(always)]
    pub(crate) fn check_relations(&self, id: Id, ctx: &QueryCtx) -> bool {
        self.checks.iter().all(|c| c.eval(id, ctx))
    }

    #[inline]
    pub(crate) fn passes_filter(&self, ctx: &QueryCtx, id: Id) -> bool {
        match &self.filter {
            None => true,
            Some(t) => {
                let r = ctx.ecs.ids.get(id).expect("edge to dead id");
                t.binary_search(&r.table).is_ok()
            }
        }
    }
}

/// One join, fully lowered.
#[derive(Debug, Clone)]
pub(crate) struct PhysicalFollow {
    pub(crate) relation: RelationId,
    pub(crate) target: Option<IdSource>,
    pub(crate) direction: Direction,
    pub(crate) scope: ScopeId,
}

impl PhysicalFollow {
    pub(crate) fn fan<'w>(&self, from: Id, ctx: &QueryCtx<'w>) -> Fan<'w> {
        let index = ctx.ecs.relations.index(self.relation);

        if let Some(pin) = self.target {
            let pin = pin.resolve(&ctx.binds, ctx.params);
            let has = match self.direction {
                Direction::Reverse => index.contains(pin, from),
                Direction::Forward | Direction::Symmetric => index.contains(from, pin),
            };
            return Fan::One(has.then_some(pin));
        }

        match self.direction {
            Direction::Forward => Fan::from(index.outgoing(from)),
            Direction::Reverse => Fan::from(index.incoming(from)),
            Direction::Symmetric => Fan::Both(index.outgoing(from), index.incoming(from)),
        }
    }
}

/// Per-row filter. Direction, target kind, negation,
/// and relationship shape are all resolved into the variant; execution
/// dispatches once on this enum and touches storage directly.
#[derive(Debug, Copy, Clone)]
pub(crate) enum PhysicalCheck {
    /// (Rel, _): any outgoing edge exists.
    AnyForward { rel: RelationId, neg: bool },
    /// (<Rel, _): any incoming edge exists (reverse index, capability-
    /// checked at lowering).
    AnyReverse { rel: RelationId, neg: bool },
    /// Edge scope_entity -> X exists. Reversed pinned checks were
    /// flipped at lowering (contains(me, X, rev) == contains(X, me, fwd)),
    /// so execution has exactly one containment orientation.
    EdgeForward { rel: RelationId, tgt: IdSource, neg: bool },
    /// Edge X -> scope_entity exists. The flipped form of [CheckMode::EdgeTo].
    EdgeReverse { rel: RelationId, tgt: IdSource, neg: bool },
}

impl PhysicalCheck {
    #[inline]
    pub(crate) fn eval(&self, src: Id, QueryCtx { ecs, binds, params, .. }: &QueryCtx) -> bool {
        let rels = &ecs.relations;
        match *self {
            PhysicalCheck::AnyForward { rel, neg } => rels.index(rel).has_outgoing(src) != neg,
            PhysicalCheck::AnyReverse { rel, neg } => rels.index(rel).has_incoming(src) != neg,
            PhysicalCheck::EdgeForward { rel, tgt, neg } => {
                let tgt = tgt.resolve(binds, params);
                rels.index(rel).contains(src, tgt) != neg
            }
            PhysicalCheck::EdgeReverse { rel, tgt, neg } => {
                let tgt = tgt.resolve(binds, params);
                rels.index(rel).contains(tgt, src) != neg
            }
        }
    }
}

/// How this follow walks the index. Fixed at lowering: `Symmetric` is
/// selected from the relation's declared topology, and `Reverse` has
/// already been proven to have a secondary index.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum Direction {
    Forward,
    Reverse,
    Symmetric,
}

/// One join's edge run for a single `from` entity.
/// Empty fans are values , not errors.
pub(crate) enum Fan<'w> {
    One(Option<Id>),
    Many(std::slice::Iter<'w, Id>),
    Both(Edges<'w>, Edges<'w>),
}

impl<'w> Fan<'w> {
    #[inline(always)]
    fn from(e: Edges<'w>) -> Self {
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
            Fan::Both(a, b) => next_edge(a).or_else(|| next_edge(b)),
        }
    }
}

#[inline(always)]
fn next_edge(e: &mut Edges<'_>) -> Option<Id> {
    match e {
        Edges::One(v) => v.take(),
        Edges::Many(ids) => match ids {
            [first, rest @ ..] => {
                *ids = rest;
                Some(*first)
            }
            _ => None,
        },
    }
}
