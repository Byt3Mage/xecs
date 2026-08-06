use std::rc::Rc;

use simple_ternary::tnr;

use crate::{
    ComponentId, Ecs, LogicalPlan,
    component::Signature,
    query::{
        access::Access,
        logical::{Filter, IdSource, LogicalCheck, LogicalFollow, LogicalScope, Relation},
        physical::{Direction, MatchedTable, PhysicalCheck, PhysicalFollow, PhysicalPlan, PhysicalScope},
    },
    relation::index::Topology,
    table_index::TableId,
};

pub fn lower_plan(ecs: &Ecs, plan: &LogicalPlan) -> PhysicalPlan {
    let LogicalPlan { access, scopes, follows } = plan;
    PhysicalPlan {
        resolved_at: ecs.generation(),
        tables: lower_tables(ecs, access, &scopes[0]),
        scopes: scopes.iter().map(|s| lower_scope(ecs, s)).collect(),
        follows: follows.iter().map(|f| lower_follow(ecs, f)).collect(),
        all_follows_injective: follows.iter().all(|f| injective(ecs, &f.relation)),
    }
}

fn lower_tables(ecs: &Ecs, access: &[Access], scope: &LogicalScope) -> Rc<[MatchedTable]> {
    let filter = &scope.filter;

    let resolve = |id| {
        let sig = &ecs.tables[id].sig;

        if !matches_signature(sig, filter) {
            return None;
        }

        access
            .iter()
            .map(|a| sig.find_id(&a.id).or_else(|| a.optional.then_some(usize::MAX)))
            .collect::<Option<_>>()
            .map(|columns| MatchedTable { id, columns })
    };

    let required = filter
        .with
        .iter()
        .chain(access.iter().filter_map(|a| (!a.optional).then_some(&a.id)));

    match seed(ecs, required) {
        None => ecs.tables.ids().filter_map(resolve).collect(),
        Some(c) => ecs.components[c].tables.iter().copied().filter_map(resolve).collect(),
    }
}

fn lower_scope(ecs: &Ecs, scope: &LogicalScope) -> PhysicalScope {
    PhysicalScope {
        filter: match_filter(ecs, &scope.filter),
        checks: scope.checks.iter().map(lower_check).collect(),
        follows: scope.follows.iter().copied().collect(),
    }
}

/// One follow, resolved against the world.
///
/// A follow *enumerates* where a check only *tests*, so the capability
/// rules differ from `lower_check`. A pinned follow degenerates to a
/// containment test (it yields the pin or nothing) and containment is
/// answerable from the primary in either direction, so pinning needs no
/// reverse index. An unpinned reversed follow has to walk the secondary,
/// so it demands one.
fn lower_follow(ecs: &Ecs, follow: &LogicalFollow) -> PhysicalFollow {
    let Relation { id: relation, target, reversed } = follow.relation;
    let direction = match ecs.relations.index(relation).topology() {
        Topology::Symmetric { .. } => Direction::Symmetric,
        Topology::Directed { .. } => tnr! {reversed =>  Direction::Reverse : Direction::Forward },
    };
    PhysicalFollow { relation, direction, target, scope: follow.scope }
}

/// One check, resolved against the world.
///
/// Pinned reversed checks flip rather than needing a reverse index:
/// `contains(me, X, reversed)` is `contains(X, me, forward)`, which the
/// primary answers. Only unpinned reversed checks ("is anything pointing
/// at me") need the secondary, and `check_capabilities` rejects those
/// upfront when it's absent.
fn lower_check(c: &LogicalCheck) -> PhysicalCheck {
    let Relation { id: rel, target, reversed } = c.relation;
    let neg = c.negated;
    match (target, reversed) {
        (None, false) => PhysicalCheck::AnyForward { rel, neg },
        (None, true) => PhysicalCheck::AnyReverse { rel, neg },
        (Some(tgt), false) => PhysicalCheck::EdgeForward { rel, tgt, neg },
        (Some(tgt), true) => PhysicalCheck::EdgeReverse { rel, tgt, neg },
    }
}

fn seed<'a>(ecs: &Ecs, required: impl Iterator<Item = &'a ComponentId>) -> Option<ComponentId> {
    required.min_by_key(|&&c| ecs.components[c].tables.len()).copied()
}

fn matches_signature(sig: &Signature, c: &Filter) -> bool {
    c.with.iter().all(|id| sig.has_id(id)) && !c.without.iter().any(|id| sig.has_id(id))
}

fn match_filter(ecs: &Ecs, c: &Filter) -> Option<Box<[TableId]>> {
    if c.empty() {
        return None;
    }

    let filter = |id: &TableId| matches_signature(&ecs.tables[*id].sig, c);

    Some(match seed(ecs, c.with.iter()) {
        Some(c) => ecs.components[c].tables.iter().copied().filter(filter).collect(),
        None => ecs.tables.ids().filter(filter).collect(),
    })
}

/// Do distinct rows bind distinct entities?
#[inline]
fn injective(ecs: &Ecs, relation: &Relation) -> bool {
    match relation.target {
        // Binds what that scope bound — injective because it is.
        Some(IdSource::Scope(_)) => true,
        // Every row binds the same id: maximal fan-in.
        Some(IdSource::Fixed(_) | IdSource::Param(_)) => false,
        None => match ecs.relations.index(relation.id).topology() {
            Topology::Symmetric { unique } => unique,
            Topology::Directed { unique_source, unique_target, .. } => {
                tnr! {relation.reversed => unique_source : unique_target}
            }
        },
    }
}
