use std::{cmp::Ordering, rc::Rc};

use crate::{
    Ecs, Id, LowerError,
    component::id::ComponentId,
    query::{
        access::{Access, AccessMode, Follows, Select},
        error::ValidationError,
        physical::{MatchedTable, PhysicalPlan},
    },
    relation::{RelationId, index::Topology},
    table_index::TableId,
};

pub type ScopeId = usize;

#[derive(Copy, Clone, Debug)]
pub struct Relation {
    pub id: RelationId,
    pub target: RelTarget,
    pub reversed: bool,
}

impl Relation {
    #[inline(always)]
    pub fn as_reversed(&self) -> Option<Self> {
        self.reversed.then(|| *self)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct RelCheck {
    pub relation: Relation,
    pub negated: bool,
}

#[derive(Copy, Clone, Debug)]
pub enum RelTarget {
    /// (Rel, _): any target at all.
    Any,
    /// (Rel, #001): a specific id, fixed at build time.
    Fixed(Id),
    /// (Rel, $n): a specific id, supplied at execution.
    Param(u16),
    /// (Rel, label): unification with an already-bound scope.
    Label(ScopeId),
}

impl RelTarget {
    #[inline]
    pub fn is_any(&self) -> bool {
        matches!(self, Self::Any)
    }
}

#[derive(Debug, Clone, Default)]
pub struct LogicalScope {
    pub access: Vec<Access>,
    pub with: Vec<ComponentId>,
    pub without: Vec<ComponentId>,
    pub rel_check: Vec<RelCheck>,
}

#[derive(Copy, Clone, Debug)]
pub struct LogicalFollow {
    pub relation: Relation,
    pub from: ScopeId,
    pub dest: ScopeId,
}

#[derive(Debug, Clone)]
pub struct LogicalPlan {
    pub(crate) scopes: Rc<[LogicalScope]>,
    pub(crate) follows: Rc<[LogicalFollow]>,
}

impl LogicalPlan {
    pub fn lower(&self, ecs: &Ecs) -> PhysicalPlan {
        PhysicalPlan {
            resolved_at: ecs.generation(),
            scopes: Box::new([]),
            follows: Box::new([]),
        }
    }
}

pub struct PlanBuilder {
    pub(crate) scopes: Vec<LogicalScope>,
    pub(crate) joins: Vec<LogicalFollow>,
    pub(crate) labels: Vec<(Rc<str>, ScopeId)>,
    current: ScopeId,
}

impl PlanBuilder {
    pub fn new() -> Self {
        Self {
            scopes: vec![],
            joins: vec![],
            labels: vec![],
            current: 0,
        }
    }
    pub fn access(mut self, access: Access) -> Self {
        self.scopes[self.current].access.push(access);
        self
    }

    pub fn with(mut self, c: ComponentId) -> Self {
        self.scopes[self.current].with.push(c);
        self
    }

    pub fn without(&mut self, c: ComponentId) -> &mut Self {
        self.scopes[self.current].without.push(c);
        self
    }

    pub fn relation_check(&mut self, filter: RelCheck) -> &mut Self {
        if let RelTarget::Label(s) = filter.relation.target {
            assert!(s != self.current, "self-unification is meaningless");
        }
        self.scopes[self.current].rel_check.push(filter);
        self
    }

    /// AS name, labels the scope being described.
    pub fn label(&mut self, name: &str) -> &mut Self {
        assert!(
            self.labels.iter().all(|(n, _)| n.as_ref() != name),
            "duplicate label `{name}`"
        );
        self.labels.push((name.into(), self.current));
        self
    }

    pub fn resolve_label(&self, name: &str) -> Option<ScopeId> {
        self.labels.iter().find_map(|(n, s)| (n.as_ref() == name).then_some(*s))
    }

    pub fn join(&mut self, relation: Relation, f: impl FnOnce(&mut Self)) -> &mut Self {
        let source = self.current;
        let target = self.scopes.len();

        // Appended BEFORE descending: joins end up in declaration order,
        // so every join's `from` scope is bound before it executes.
        self.scopes.push(LogicalScope::default());
        self.joins.push(LogicalFollow { relation, from: source, dest: target });

        let (current, labels) = (self.current, self.labels.len());
        self.current = target;
        f(self);
        self.current = current;
        self.labels.truncate(labels);
        self
    }

    pub fn build(self) -> LogicalPlan {
        LogicalPlan {
            scopes: self.scopes.into(),
            follows: self.joins.into(),
        }
    }
}

pub fn validate_scope<C, J>(ecs: &Ecs, plan: &LogicalPlan, scope: ScopeId) -> Result<(), ValidationError>
where
    C: Select,
    J: Follows,
{
    let requests = C::describe();
    let declared = &plan.scopes[scope].access;

    if requests.len() != declared.len() {
        return Err(ValidationError::ColumnArity {
            scope,
            received: requests.len(),
            expected: declared.len(),
        });
    }

    for (index, (req, dec)) in requests.iter().zip(declared).enumerate() {
        use AccessMode::*;
        match (req.mode, dec.mode) {
            (Read, Read) | (Write, Write) => {}
            (Read, Write) => return Err(ValidationError::ReadOnWrite { scope, index, name: req.type_name }),
            (Write, Read) => return Err(ValidationError::WriteOnRead { scope, index, name: req.type_name }),
        }

        match (req.optional, dec.optional) {
            (true, true) | (false, false) => {}
            (true, false) => return Err(ValidationError::OptionalOnRequired { scope, index, name: req.type_name }),
            (false, true) => return Err(ValidationError::RequiredOnOptional { scope, index, name: req.type_name }),
        }

        let meta = &ecs.components.get(dec.id).meta;
        todo!("use layout fingerprint");
    }

    Ok(())
}

fn match_scope(LogicalScope { access, with, without, .. }: &LogicalScope, ecs: &Ecs) -> Box<[MatchedTable]> {
    if access.is_empty() && with.is_empty() {
        // Every table matches, so we remove tables that have 'without' components.
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
    let scopes = logical.scopes.iter();
    let follows = logical.follows.iter().filter_map(|j| j.relation.as_reversed());
    let checks = scopes.flat_map(|s| s.rel_check.iter().filter_map(|c| c.relation.as_reversed()));

    for rel in follows.chain(checks) {
        let index = ecs.relations.index(rel.id);

        // Symmetric edges must have no direction.
        if index.props().topology.is_symmetric() {
            return Err(LowerError::ReversedSymmetric(rel.id));
        }

        // Reversed enumeration walks the reverse index.
        // So reversed relationships without a pinned target must have one.
        if rel.target.is_any() && !index.has_reverse() {
            return Err(LowerError::NoReverseIndex(rel.id));
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
    logical.follows.iter().any(|j| {
        let connected = (j.from == x && j.dest == y) || (j.from == y && j.dest == x);
        connected && ecs.relations.index(j.relation.id).props().acyclic
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
