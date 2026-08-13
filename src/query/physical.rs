use std::rc::Rc;

use crate::{
    Id,
    query::{
        context::QueryCtx,
        logical::{Direction, FollowId, IdSource, ScopeId},
    },
    relation::{
        id::RelationId,
        storage::{EdgeIter, Edges},
    },
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
    pub(crate) fn fan<'w>(&self, from: Id, ctx: &QueryCtx<'w>) -> EdgeIter<'w> {
        let rel = &ctx.ecs.relations[self.relation];
        match self.target {
            Some(pin) => {
                let pin = pin.resolve(&ctx.binds, ctx.params);
                EdgeIter::One(match self.direction {
                    Direction::Forward => rel.contains(from, pin).then_some(pin),
                    Direction::Reverse => rel.contains(pin, from).then_some(pin),
                })
            }
            None => match self.direction {
                Direction::Forward => rel.outgoing(from),
                Direction::Reverse => rel.incoming(from),
            }
            .map_or_else(EdgeIter::empty, Edges::into_iter),
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
    /// Edge X -> scope_entity exists. The flipped form of [Self::EdgeForward].
    EdgeReverse { rel: RelationId, tgt: IdSource, neg: bool },
}

impl PhysicalCheck {
    #[inline]
    pub(crate) fn eval(&self, src: Id, QueryCtx { ecs, binds, params, .. }: &QueryCtx) -> bool {
        let rels = &ecs.relations;
        match *self {
            PhysicalCheck::AnyForward { rel, neg } => rels[rel].has_outgoing(src) != neg,
            PhysicalCheck::AnyReverse { rel, neg } => rels[rel].has_incoming(src) != neg,
            PhysicalCheck::EdgeForward { rel, tgt, neg } => {
                let tgt = tgt.resolve(binds, params);
                rels[rel].contains(src, tgt) != neg
            }
            PhysicalCheck::EdgeReverse { rel, tgt, neg } => {
                let tgt = tgt.resolve(binds, params);
                rels[rel].contains(tgt, src) != neg
            }
        }
    }
}
